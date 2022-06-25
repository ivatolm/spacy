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

    pub fn new(main_event_channel_tx: mpsc::Sender<proto_msg::Event>) -> Self {
        let fsm = FSM::new(0, HashMap::from([
            (0, vec![1]),
            (1, vec![2, 3]),
            (2, vec![1]),
            (3, vec![1])
        ]));

        let (event_channel_tx, event_channel_rx) = mpsc::channel();

        let (listener_event_channel_tx, listener_event_channel_rx) = mpsc::channel();
        let server_event_channel_tx = event_channel_tx.clone();

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
        let mut servers = vec![];
        for ip in utils::get_ipv4_ips() {
            match TcpListener::bind((ip, 32000)) {
                Ok(listener) => {
                    let fd = listener.as_raw_fd();
                    servers.push(fd);
                    self.servers.insert(fd, listener);

                    let event = proto_msg::Event {
                        dir: proto_msg::event::Direction::Internal as i32,
                        kind: proto_msg::event::Kind::NewFd as i32,
                        data: vec![fd.to_ne_bytes().to_vec()]
                    };
                    self.listener_event_channel_tx.send(event).unwrap();
                },
                Err(_) => {
                    todo!("Properly log the error");
                }
            }
        }

        self.fsm.transition(1)?;
        Ok(())
    }

    fn wait_event(&mut self) -> Result<(), ServerError> {
        let event = match self.event_channel_rx.recv() {
            Ok(event) => event,
            Err(_) => {
                todo!("Properly log the error");
            }
        };

        let event_direction = event.dir;
        self.fsm.push_event(event);

        match event_direction {
            0 => self.fsm.transition(2)?,
            1 => self.fsm.transition(3)?,
            _ => {
                todo!("Properly log the error");
            }
        }

        Ok(())
    }

    fn handle_incoming_event(&mut self) -> Result<(), ServerError> {
        let event = self.fsm.pop_event().unwrap();

        if event.kind == proto_msg::event::Kind::NewStreamEvent as i32 {
            let bytes = event.data.get(0).unwrap();
            let fd = utils::i32_from_ne_bytes_vec(bytes.to_vec()).unwrap();

            if let Some(listener) = self.servers.get(&fd) {
                match listener.accept() {
                    Ok((stream, _addr)) => {
                        // Log new connection
                        self.clients.insert(fd, stream);

                        self.listener_event_channel_tx.send(proto_msg::Event {
                            dir: proto_msg::event::Direction::Internal as i32,
                            kind: proto_msg::event::Kind::NewFd as i32,
                            data: vec![fd.to_ne_bytes().to_vec()]
                        }).unwrap();
                    },
                    Err(_) => {
                        todo!("Properly log the error");
                    }
                };
            }
        }

        self.main_event_channel_tx.send(event).unwrap();

        self.fsm.transition(1)?;
        Ok(())
    }

    fn handle_outcoming_event(&mut self) -> Result<(), ServerError> {
        println!("Sending an outcoming event");

        self.fsm.transition(1)?;
        Ok(())
    }

    fn stop(self) -> Result<(), ServerError> {
        match self.listener_handle.join() {
            Ok(_) => Ok(()),
            Err(_) => Err(ServerError::InternalError)
        }
    }

    fn t_listener(listener_rx: mpsc::Receiver<proto_msg::Event>,
                  server_tx: mpsc::Sender<proto_msg::Event>) {
        let mut readfds_vec = vec![];

        loop {
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

            for event in incoming_events {
                if event.kind == proto_msg::event::Kind::NewFd as i32 {
                    let bytes = event.data.get(0).unwrap();
                    let fd = utils::i32_from_ne_bytes_vec(bytes.to_vec()).unwrap();
                    readfds_vec.push(fd);
                }
            }

            let mut readfds = FdSet::new();
            for fd in readfds_vec.iter() {
                readfds.insert(*fd);
            }

            let mut timeout = TimeVal::milliseconds(100);
            let result = select(None, &mut readfds, None, None, &mut timeout);
            match result {
                Ok(ready_count) => {
                    // log ready count
                    ready_count
                },
                Err(_) => {
                    todo!("Properly log the error");
                }
            };

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
