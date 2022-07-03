use std::{
    collections::HashMap,
    net::{TcpListener, TcpStream},
    os::unix::prelude::AsRawFd,
    sync::mpsc,
    thread,
    time
};
use nix::sys::{
    select::{select, FdSet},
    time::{TimeVal, TimeValLike}
};
use common::{
    fsm::{FSM, FSMError},
    event::proto_msg,
    utils
};

pub struct Server {
    fsm: FSM,
    event_channel_tx: mpsc::Sender<proto_msg::Event>,
    event_channel_rx: mpsc::Receiver<proto_msg::Event>,
    servers: HashMap<i32, TcpListener>,
    clients: HashMap<i32, TcpStream>,
    listener_event_channel_tx: mpsc::Sender<proto_msg::Event>,
    listener_handle: thread::JoinHandle<()>,

    main_event_channel_tx: mpsc::Sender<proto_msg::Event>
}

pub enum ServerError {
    InternalError
}

impl Server {
    // 0 - initialization
    // 1 - waiting for events
    // 2 - handling incoming event
    // 3 - handling outcoming event
    // 4 - stop

    pub fn new(main_event_channel_tx: mpsc::Sender<proto_msg::Event>) -> Self {
        let fsm = FSM::new(0, HashMap::from([
            (0, vec![1, 4]),
            (1, vec![2, 3, 4]),
            (2, vec![1, 4]),
            (3, vec![1, 4]),
            (4, vec![])
        ]));

        // Creating main event communication channel
        let (event_channel_tx, event_channel_rx) = mpsc::channel();

        // Creating communication channel with `listener`
        let (listener_event_channel_tx, listener_event_channel_rx) = mpsc::channel();
        let server_event_channel_tx = event_channel_tx.clone();

        // Spawning `listener`
        let listener_handle = thread::spawn(move || {
            Self::t_listener(listener_event_channel_rx, server_event_channel_tx);
        });

        Self {
            fsm,
            event_channel_tx,
            event_channel_rx,
            servers: HashMap::new(),
            clients: HashMap::new(),
            listener_event_channel_tx,
            listener_handle,

            main_event_channel_tx
        }
    }

    pub fn start(mut self) -> (mpsc::Sender<proto_msg::Event>,
                               thread::JoinHandle<Result<(), ServerError>>) {
        let event_channel_tx_clone = self.event_channel_tx.clone();

        // Starting FSM loop
        let handle = thread::spawn(move || loop {
            match self.fsm.state {
                0 => self.init()?,
                1 => self.wait_event()?,
                2 => self.handle_incoming_event()?,
                3 => self.handle_outcoming_event()?,
                4 => {
                    self.stop()?;
                    return Ok(())
                },
                _ => unreachable!()
            }
        });

        (event_channel_tx_clone, handle)
    }

    fn init(&mut self) -> Result<(), ServerError> {
        log::debug!("State `init`");

        // For each avaliable ip creating a listener(server)
        // TODO: Add support for ipv6 ips
        let mut servers = vec![];
        for ip in utils::get_ipv4_ips() {
            match TcpListener::bind((ip, 32000)) {
                Ok(listener) => {
                    log::info!("Started listener {}", listener.local_addr().unwrap());

                    // Adding a new server to servers-list
                    let fd = listener.as_raw_fd();
                    servers.push(fd);
                    self.servers.insert(fd, listener);

                    // Notifing `listener` thread about new server
                    let event = proto_msg::Event {
                        dir: proto_msg::event::Direction::Internal as i32,
                        kind: proto_msg::event::Kind::NewFd as i32,
                        data: vec![fd.to_ne_bytes().to_vec()]
                    };
                    self.listener_event_channel_tx.send(event).unwrap();
                },
                Err(error) => {
                    log::warn!("Couldn't start a listener {}: {}", ip, error);
                }
            }
        }

        self.fsm.transition(1)?;
        Ok(())
    }

    fn wait_event(&mut self) -> Result<(), ServerError> {
        log::debug!("State `wait_event`");

        // Waiting for new events
        let event = match self.event_channel_rx.recv() {
            Ok(event) => event,
            Err(error) => {
                log::error!("Error occured while received event: {}", error);
                panic!();
            }
        };

        let event_direction = event.dir;
        self.fsm.push_event(event);

        // Handling event based on it's direction
        match event_direction {
            // incoming
            0 => {
                log::debug!("Received `incoming` event");

                self.fsm.transition(2)?;
            },
            // outcoming
            1 => {
                log::debug!("Received `outcoming` event");

                self.fsm.transition(3)?;
            },
            _ => {
                log::warn!("Received event with the unknown direction");
            }
        }

        Ok(())
    }

    fn handle_incoming_event(&mut self) -> Result<(), ServerError> {
        log::debug!("State `handle_incoming_event`");

        let event = self.fsm.pop_event().unwrap();

        // If we got new event from `listener` thread
        if event.kind == proto_msg::event::Kind::NewStreamEvent as i32 {
            // Parsing fd from the event
            let bytes = event.data.get(0).unwrap();
            let fd = utils::i32_from_ne_bytes(bytes).unwrap();

            // If it's a fd of a server, than accept new client
            if let Some(listener) = self.servers.get(&fd) {
                match listener.accept() {
                    Ok((stream, addr)) => {
                        log::info!("New client connected {}", addr);

                        self.clients.insert(fd, stream);

                        // Notify `listener` thread about new client
                        self.listener_event_channel_tx.send(proto_msg::Event {
                            dir: proto_msg::event::Direction::Internal as i32,
                            kind: proto_msg::event::Kind::NewFd as i32,
                            data: vec![fd.to_ne_bytes().to_vec()]
                        }).unwrap();
                    },
                    Err(error) => {
                        log::warn!("Couldn't connect new client: {}", error);
                    }
                };
            }
        }

        // Send an event to main
        self.main_event_channel_tx.send(event).unwrap();

        self.fsm.transition(1)?;
        Ok(())
    }

    fn handle_outcoming_event(&mut self) -> Result<(), ServerError> {
        log::debug!("State `handle_outcoming_event`");

        self.fsm.transition(1)?;
        Ok(())
    }

    fn stop(self) -> Result<(), ServerError> {
        log::debug!("State `stop`");

        match self.listener_handle.join() {
            Ok(_) => Ok(()),
            Err(_) => Err(ServerError::InternalError)
        }
    }

    fn t_listener(listener_rx: mpsc::Receiver<proto_msg::Event>,
                  server_tx: mpsc::Sender<proto_msg::Event>) {
        log::debug!("Listener thread started");

        let mut readfds_vec = vec![];

        loop {
            // Wait for events from other thread
            let mut incoming_events: Vec<proto_msg::Event> = vec![];
            loop {
                let timeout = time::Duration::from_millis(100);
                match listener_rx.recv_timeout(timeout) {
                    Ok(event) => {
                        incoming_events.push(event);
                    }
                    Err(_) => break
                }
            }

            // Handle received events
            for event in incoming_events {
                if event.kind == proto_msg::event::Kind::NewFd as i32 {
                    let bytes = event.data.get(0).unwrap();
                    let fd = utils::i32_from_ne_bytes(bytes).unwrap();
                    readfds_vec.push(fd);
                }
            }

            // Update set of fds to be read
            let mut readfds = FdSet::new();
            for fd in readfds_vec.iter() {
                readfds.insert(*fd);
            }

            // Wait for acitivy on specified fds
            let mut timeout = TimeVal::milliseconds(100);
            let result = select(None, &mut readfds, None, None, &mut timeout);
            if result.is_err() {
                log::warn!("`select` exited with error: {:?}", result.err());
            }

            // Send events to the handler
            for fd in readfds.fds(None) {
                let event = proto_msg::Event {
                    dir: proto_msg::event::Direction::Internal as i32,
                    kind: proto_msg::event::Kind::NewStreamEvent as i32,
                    data: vec![fd.to_ne_bytes().to_vec()]
                };

                server_tx.send(event).unwrap();
            }
        }
    }
}

impl From<FSMError> for ServerError {
    fn from(_: FSMError) -> Self {
        ServerError::InternalError
    }
}
