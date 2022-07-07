use std::{
    collections::HashMap,
    net::{TcpListener, TcpStream, IpAddr, Ipv4Addr, SocketAddr, Shutdown},
    os::unix::prelude::AsRawFd,
    sync::mpsc,
    thread,
    time, io::Read
};
use nix::{sys::{
    select::{select, FdSet},
    time::{TimeVal, TimeValLike}
}, unistd::read};
use common::{
    fsm::{FSM, FSMError},
    event::{proto_msg, self},
    utils
};

pub struct Server {
    fsm: FSM,
    event_channel_tx: mpsc::Sender<proto_msg::Event>,
    event_channel_rx: mpsc::Receiver<proto_msg::Event>,
    stream_channel_rx: mpsc::Receiver<TcpStream>,
    servers: HashMap<i32, TcpListener>,
    clients: HashMap<i32, TcpStream>,
    nodes: HashMap<i32, TcpStream>,
    listener_event_channel_tx: mpsc::Sender<proto_msg::Event>,
    listener_handle: thread::JoinHandle<()>,
    scanner_handle: thread::JoinHandle<()>,

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

		// Creating communication channel with `scanner`
		let (scanner_stream_channel_tx, stream_channel_rx) = mpsc::channel();
        let server_event_channel_tx = event_channel_tx.clone();

		// Spawning `scanner`
		let scanner_handle = thread::spawn(move || {
			Self::t_scanner(server_event_channel_tx, scanner_stream_channel_tx);
		});

        Self {
            fsm,
            event_channel_tx,
            event_channel_rx,
			stream_channel_rx,
            servers: HashMap::new(),
            clients: HashMap::new(),
            nodes: HashMap::new(),
            listener_event_channel_tx,
            listener_handle,
			scanner_handle,

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
        if event_direction == proto_msg::event::Direction::Incoming as i32 {
            log::debug!("Received `incoming` event");

            self.fsm.transition(2)?;
            return Ok(());
        }

        if event_direction == proto_msg::event::Direction::Outcoming as i32 {
            log::debug!("Received `outcoming` event");

            self.fsm.transition(3)?;
            return Ok(());
        }

        log::warn!("Received event with the unknown direction");

        Ok(())
    }

    fn handle_incoming_event(&mut self) -> Result<(), ServerError> {
        log::debug!("State `handle_incoming_event`");

        let event = self.fsm.pop_event().unwrap();
        let mut event_to_main = None;

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

                        let client_fd = stream.as_raw_fd();
                        self.clients.insert(client_fd, stream);

                        // Notify `listener` thread about new client
                        self.listener_event_channel_tx.send(proto_msg::Event {
                            dir: proto_msg::event::Direction::Internal as i32,
                            kind: proto_msg::event::Kind::NewFd as i32,
                            data: vec![client_fd.to_ne_bytes().to_vec()]
                        }).unwrap();
                    },
                    Err(error) => {
                        log::warn!("Couldn't connect new client: {}", error);
                    }
                };
            }

            // If it's a fd of a client, that read from stream
            if let Some(mut stream) = self.clients.get(&fd) {
                let mut message = vec![];

                // Reading stream until read
                let mut buf = [0u8; 1024];
                loop {
                    let bytes_num = stream.read(&mut buf).unwrap();
                    message.extend(&buf[0..bytes_num]);

                    if bytes_num < 1024 {
                        break;
                    }

                    buf = [0u8; 1024];
                }


                // If we read zero bytes, in TCP it means that client disconnected
                // else we just have got a new event from the client
                if message.len() == 0 {
                    let client_fd = fd;

                    log::info!("Client disconnected {}", stream.peer_addr().unwrap());

                    // Shutting down the stream
                    stream.shutdown(Shutdown::Both).unwrap();

                    // Removing client from client-list
                    self.clients.remove(&client_fd);

                    // Notify `listener` thread about old client
                    self.listener_event_channel_tx.send(proto_msg::Event {
                        dir: proto_msg::event::Direction::Internal as i32,
                        kind: proto_msg::event::Kind::OldFd as i32,
                        data: vec![client_fd.to_ne_bytes().to_vec()]
                    }).unwrap();
                } else {
                    log::info!("Received new {}-bytes message from the client", message.len());

                    // Setting up an event to be sent to main
                    event_to_main = Some(event::deserialize(&message).unwrap());
                }
            }
        }

        // If we got new event from `scanner` thread
        if event.kind == proto_msg::event::Kind::NewStream as i32 {
            log::debug!("Scanner found new node");
            // Reading sent stream and adding it to the list of nodes
            let stream = self.stream_channel_rx.recv().unwrap();
            let addr = stream.peer_addr().unwrap();
            log::info!("New node connected {}", addr);

            let fd = stream.as_raw_fd();
            self.nodes.insert(fd, stream);

            // Notify `listener` thread about new client
            self.listener_event_channel_tx.send(proto_msg::Event {
                dir: proto_msg::event::Direction::Internal as i32,
                kind: proto_msg::event::Kind::NewFd as i32,
                data: vec![fd.to_ne_bytes().to_vec()]
            }).unwrap();
        }

        // Send an event to main
        if let Some(event) = event_to_main {
            self.main_event_channel_tx.send(event).unwrap();
        }

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
        }?;

        match self.scanner_handle.join() {
            Ok(_) => Ok(()),
            Err(_) => Err(ServerError::InternalError)
        }?;

        Ok(())
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

                if event.kind == proto_msg::event::Kind::OldFd as i32 {
                    let bytes = event.data.get(0).unwrap();
                    let fd = utils::i32_from_ne_bytes(bytes).unwrap();

                    let mut fd_index = 0;
                    for i in 0..readfds_vec.len() {
                        if *readfds_vec.get(i).unwrap() == fd {
                            fd_index = i;
                            break;
                        }
                    }
                    readfds_vec.remove(fd_index);
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
                    dir: proto_msg::event::Direction::Incoming as i32,
                    kind: proto_msg::event::Kind::NewStreamEvent as i32,
                    data: vec![fd.to_ne_bytes().to_vec()]
                };

                server_tx.send(event).unwrap();
            }
        }
    }

    fn t_scanner(server_event_tx: mpsc::Sender<proto_msg::Event>,
				 server_stream_tx: mpsc::Sender<TcpStream>) {

        // Get this machine ips (to not ping them)
        let local_ips = utils::get_ipv4_ips();

        // Get proper interfaces
        for (network, _mask) in utils::get_networks_and_masks().iter() {
            // Get octets of networks
            if !network.is_ipv4() {
                continue;
            }

            // TODO: Add proper check
            let octets = utils::get_octets(network).unwrap();

            // Ping all avaliable IPs (not really, but OK for MVP)
            for i in 0..255 {
                let ip = IpAddr::V4(Ipv4Addr::new(octets[0], octets[1], octets[2], i));

                // We dont want to ping ourselves
                if local_ips.contains(&ip) {
                    // TODO: remove comment. Only for one-machine testing
                    // continue;
                }

                // TODO: 32001 -> 32000. Only for one-machine testing
                let socket_address = SocketAddr::new(ip, 32001);
                let stream = TcpStream::connect_timeout(
                    &socket_address,
                    time::Duration::from_millis(100)
                );

                match stream {
                    Ok(stream) => {
                        // Send stream of client that responded
						server_stream_tx.send(stream).unwrap();
						// Notify server, that new client detected
						server_event_tx.send(proto_msg::Event {
							dir: proto_msg::event::Direction::Incoming as i32,
							kind: proto_msg::event::Kind::NewStream as i32,
							data: vec![]
						}).unwrap();
                    },
                    Err(_) => {
                        // Doing nothing, this situation is perfectly OK
                    }
                }
            }
        }
    }
}

impl From<FSMError> for ServerError {
    fn from(_: FSMError) -> Self {
        ServerError::InternalError
    }
}
