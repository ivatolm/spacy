use std::{
    collections::HashMap,
    net::{TcpListener, TcpStream, IpAddr, Ipv4Addr, SocketAddr, Shutdown},
    os::unix::prelude::AsRawFd,
    sync::{mpsc, Arc, Mutex},
    thread,
    time, io::Write
};
use nix::sys::{
    select::{select, FdSet},
    time::{TimeVal, TimeValLike}
};
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
    nodes_ips: Arc<Mutex<HashMap<IpAddr, i32>>>,
    nodes_ids: HashMap<u128, i32>,
    readfds: Vec<i32>,
    scanner_handle: thread::JoinHandle<()>,

    node_id: u128,

    main_event_channel_tx: mpsc::Sender<proto_msg::Event>
}

pub enum ServerError {
    InternalError
}

impl Server {
    // 0 - initialization
    // 1 - waiting for events
    // 2 - handling event
    // 3 - handling incoming event
    // 4 - handling outcoming event
    // 5 - stop

    pub fn new(main_event_channel_tx: mpsc::Sender<proto_msg::Event>, node_id: u128) -> Self {
        let fsm = FSM::new(0, HashMap::from([
            (0, vec![1, 5]),
            (1, vec![2, 5]),
            (2, vec![1, 3, 4, 5]),
            (3, vec![2, 5]),
            (4, vec![2, 5]),
            (5, vec![])
        ]));

        // Creating main event communication channel
        let (event_channel_tx, event_channel_rx) = mpsc::channel();

		// Creating communication channel with `scanner`
		let (scanner_stream_channel_tx, stream_channel_rx) = mpsc::channel();
        let server_event_channel_tx = event_channel_tx.clone();

		// Spawning `scanner`
        let nodes_ips = Arc::new(Mutex::new(HashMap::new()));
        let known_nodes_ips = nodes_ips.clone();
		let scanner_handle = thread::spawn(move || {
			Self::t_scanner(server_event_channel_tx,
                            scanner_stream_channel_tx,
                            known_nodes_ips,
                            node_id);
		});

        Self {
            fsm,
            event_channel_tx,
            event_channel_rx,
			stream_channel_rx,
            servers: HashMap::new(),
            clients: HashMap::new(),
            nodes: HashMap::new(),
            nodes_ips,
            nodes_ids: HashMap::new(),
            readfds: vec![],
			scanner_handle,

            node_id,

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
                2 => self.handle_event()?,
                3 => self.handle_incoming_event()?,
                4 => self.handle_outcoming_event()?,
                5 => {
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
                        dir: Some(proto_msg::event::Dir::Incoming as i32),
                        dest: None,
                        kind: proto_msg::event::Kind::NewFd as i32,
                        data: vec![fd.to_ne_bytes().to_vec()],
                        meta: vec![]
                    };
                    self.event_channel_tx.send(event).unwrap();
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
        // log::debug!("State `wait_event`");

        // Waiting for new events
        match self.event_channel_rx.try_recv() {
            Ok(event) => {
                if event.kind == proto_msg::event::Kind::NewFd as i32 {
                    let bytes = event.data.get(0).unwrap();
                    let fd = utils::i32_from_ne_bytes(bytes).unwrap();
                    self.readfds.push(fd);
                }

                else if event.kind == proto_msg::event::Kind::OldFd as i32 {
                    let bytes = event.data.get(0).unwrap();
                    let fd = utils::i32_from_ne_bytes(bytes).unwrap();

                    let mut fd_index = 0;
                    for i in 0..self.readfds.len() {
                        if *self.readfds.get(i).unwrap() == fd {
                            fd_index = i;
                            break;
                        }
                    }
                    self.readfds.remove(fd_index);
                }

                else{
                    self.fsm.push_event(event);
                }
            },
            Err(_) => {}
        };

        let mut readfds = FdSet::new();
        for fd in self.readfds.iter() {
            readfds.insert(*fd);
        }

        // Wait for acitivy on specified fds
        let mut timeout = TimeVal::milliseconds(1);
        let result = select(None, &mut readfds, None, None, &mut timeout);
        if result.is_err() {
            log::warn!("`select` exited with error: {:?}", result.err());
        }

        // Send events to the handler
        for fd in readfds.fds(None) {
            let event = proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Incoming as i32),
                dest: None,
                kind: proto_msg::event::Kind::NewStreamEvent as i32,
                data: vec![fd.to_ne_bytes().to_vec()],
                meta: vec![]
            };

            self.fsm.push_event(event);
        }

        self.fsm.transition(2)?;
        Ok(())
    }

    fn handle_event(&mut self) -> Result<(), ServerError> {
        // log::debug!("State `handle_event`");

        let event = match self.fsm.pop_front_event() {
            Some(event) => event,
            None => {
                self.fsm.transition(1)?;
                return Ok(());
            }
        };
        let event_direction = event.dir;
        self.fsm.push_front_event(event);

        // Handling event based on it's direction
        if let Some(dir) = event_direction {
            if dir == proto_msg::event::Dir::Incoming as i32 {
                log::debug!("Received `incoming` event");

                self.fsm.transition(3)?;
            }

            else if dir == proto_msg::event::Dir::Outcoming as i32 {
                log::debug!("Received `outcoming` event");

                self.fsm.transition(4)?;
            }

            else {
                log::warn!("Received event with the unknown direction");
            }
        }

        Ok(())
    }

    fn handle_incoming_event(&mut self) -> Result<(), ServerError> {
        log::debug!("State `handle_incoming_event`");

        let event = self.fsm.pop_front_event().unwrap();

        // If we got new event from `listener` thread
        if event.kind == proto_msg::event::Kind::NewStreamEvent as i32 {
            log::debug!("Handling `new_stream_event`");

            // Parsing fd from the event
            let bytes = event.data.get(0).unwrap();
            let fd = utils::i32_from_ne_bytes(bytes).unwrap();

            // Matching fd to handler
            if self.servers.contains_key(&fd) {
                self.handle_new_stream_event_server(fd)?;
            }

            else if self.clients.contains_key(&fd) {
                self.handle_new_stream_event_client(fd)?;
            }

            else if self.nodes.contains_key(&fd) {
                self.handle_new_stream_event_node(fd)?;
            }

            else {
                log::warn!("There is no handler for this fd: {}", fd);
            }
        }

        // If we got new event from `scanner` thread
        else if event.kind == proto_msg::event::Kind::NewStream as i32 {
            self.handle_new_stream(event)?;
        }

        else {
            log::warn!("Received event with unknown kind: {}", event.kind);
        }

        self.fsm.transition(2)?;
        Ok(())
    }

    fn handle_outcoming_event(&mut self) -> Result<(), ServerError> {
        log::debug!("State `handle_outcoming_event`");

        let event = self.fsm.pop_front_event().unwrap();

        // Broadcasting some event
        if event.kind == proto_msg::event::Kind::BroadcastEvent as i32 {
            self.handle_broadcast_event(event)?;
        }

        // Approving transaction
        else if event.kind == proto_msg::event::Kind::ApproveTransaction as i32 {
            self.handle_approve_transaction(event)?;
        }

        // Send a response to a client
        else if event.kind == proto_msg::event::Kind::RespondClient as i32 {
            self.handle_respond_client(event)?;
        }

        else {
            log::warn!("Received event with unknown kind: {}", event.kind);
        }

        self.fsm.transition(2)?;
        Ok(())
    }

    fn stop(self) -> Result<(), ServerError> {
        log::debug!("State `stop`");

        match self.scanner_handle.join() {
            Ok(_) => Ok(()),
            Err(_) => Err(ServerError::InternalError)
        }?;

        Ok(())
    }

    fn handle_new_stream_event_server(&mut self, fd: i32) -> Result<(), ServerError> {
        log::debug!("Handling `new_stream_event` from server");

        let listener = self.servers.get(&fd).unwrap();

        match listener.accept() {
            Ok((mut stream, addr)) => {
                // Getting stream's fd
                let new_fd = stream.as_raw_fd();

                // Handshake
                let events = utils::read_events(&mut stream).unwrap();
                // Assuming we will get only one event
                let handshake_event = events.get(0).unwrap();

                let mut handshake_ok = true;
                if handshake_event.kind == proto_msg::event::Kind::MarkMeClient as i32 {
                    log::info!("New client connected {}", addr);
                    self.clients.insert(new_fd, stream);
                }

                else if handshake_event.kind == proto_msg::event::Kind::MarkMeNode as i32 {
                    {
                        let mut nodes_ips = self.nodes_ips.lock().unwrap();

                        if !nodes_ips.contains_key(&addr.ip()) {
                            log::info!("New node connected {}", addr);

                            // Notify `node` about new connection
                            self.main_event_channel_tx.send(proto_msg::Event {
                                dir: Some(proto_msg::event::Dir::Incoming as i32),
                                dest: Some(proto_msg::event::Dest::Node as i32),
                                kind: proto_msg::event::Kind::NodeConnected as i32,
                                data: handshake_event.data.clone(),
                                meta: vec![]
                            }).unwrap();

                            // Respond with this node id
                            let event = proto_msg::Event {
                                dir: None,
                                dest: None,
                                kind: 0,
                                data: vec![self.node_id.to_ne_bytes().to_vec()],
                                meta: vec![]
                            };
                            stream.write(&event::serialize(event)).unwrap();

                            self.nodes.insert(new_fd, stream);
                            nodes_ips.insert(addr.ip(), fd);

                            let bytes = handshake_event.data.get(0).unwrap();
                            let node_id = utils::u128_from_ne_bytes(bytes).unwrap();

                            self.nodes_ids.insert(node_id, new_fd);
                        } else {
                            return Ok(());
                        }
                    }
                }

                else {
                    stream.shutdown(Shutdown::Both).unwrap();
                    handshake_ok = false;
                }

                if handshake_ok {
                    // Notify `listener` thread about new fd
                    self.event_channel_tx.send(proto_msg::Event {
                        dir: Some(proto_msg::event::Dir::Incoming as i32),
                        dest: None,
                        kind: proto_msg::event::Kind::NewFd as i32,
                        data: vec![new_fd.to_ne_bytes().to_vec()],
                        meta: vec![]
                    }).unwrap();
                } else {
                    log::warn!("Handshake failed");
                }
            },
            Err(error) => {
                log::warn!("Couldn't accept new connection: {}", error);
            }
        };

        Ok(())
    }

    fn handle_new_stream_event_client(&mut self, fd: i32) -> Result<(), ServerError> {
        log::debug!("Handling `new_stream_event` from client");

        let stream = self.clients.get_mut(&fd).unwrap();

        // Getting sent events
        let events = utils::read_events(stream).unwrap();
        if events.len() != 0 {
            log::debug!("Received new {}-event message from the client", events.len());

            for event in events {
                let event = proto_msg::Event {
                    dir: Some(proto_msg::event::Dir::Incoming as i32),
                    dest: Some(proto_msg::event::Dest::PluginMan as i32),
                    kind: event.kind,
                    data: event.data,
                    meta: vec![fd.to_ne_bytes().to_vec()]
                };

                self.main_event_channel_tx.send(event).unwrap();
            }
        }

        else {
            log::info!("Client disconnected {}", stream.peer_addr().unwrap());

            // Disconnecting client
            stream.shutdown(Shutdown::Both).unwrap();
            self.clients.remove(&fd);

            // Notify `listener` thread about old fd
            self.event_channel_tx.send(proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Incoming as i32),
                dest: None,
                kind: proto_msg::event::Kind::OldFd as i32,
                data: vec![fd.to_ne_bytes().to_vec()],
                meta: vec![]
            }).unwrap();
        }

        Ok(())
    }

    fn handle_new_stream_event_node(&mut self, fd: i32) -> Result<(), ServerError> {
        log::debug!("Handling `new_stream_event` from node");

        let stream = self.nodes.get_mut(&fd).unwrap();

        // Getting sent events
        let events = utils::read_events(stream).unwrap();
        if events.len() != 0 {
            log::debug!("Received new {}-event message from the node", events.len());

            for event in events {
                // Adding fd to event's meta information
                let mut meta = event.meta;
                meta.insert(0, fd.to_ne_bytes().to_vec());

                let event = proto_msg::Event {
                    dir: Some(proto_msg::event::Dir::Incoming as i32),
                    dest: Some(proto_msg::event::Dest::Node as i32),
                    kind: event.kind,
                    data: event.data,
                    meta
                };

                self.main_event_channel_tx.send(event).unwrap();
            }
        }

        else {
            log::info!("Node disconnected {}", stream.peer_addr().unwrap());

            // Disconnecting node
            let addr = stream.peer_addr().unwrap();
            stream.shutdown(Shutdown::Both).unwrap();
            self.nodes.remove(&fd);

            {
                let mut nodes_ips = self.nodes_ips.lock().unwrap();
                nodes_ips.remove(&addr.ip());
            }

            let nodes_ids_clone = self.nodes_ids.clone();

            let mut id_to_del = None;
            for (node_id, node_fd) in nodes_ids_clone.iter() {
                if fd == *node_fd {
                    id_to_del = Some(node_id);
                    break;
                }
            }

            if let Some(id) = id_to_del {
                self.nodes_ids.remove(&id);
            }

            // Notify `node` about old connection
            self.main_event_channel_tx.send(proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Incoming as i32),
                dest: Some(proto_msg::event::Dest::Node as i32),
                kind: proto_msg::event::Kind::NodeDisconnected as i32,
                data: vec![id_to_del.unwrap().to_ne_bytes().to_vec()],
                meta: vec![]
            }).unwrap();

            // Notify `listener` thread about old client
            self.event_channel_tx.send(proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Incoming as i32),
                dest: None,
                kind: proto_msg::event::Kind::OldFd as i32,
                data: vec![fd.to_ne_bytes().to_vec()],
                meta: vec![]
            }).unwrap();
        }

        Ok(())
    }

    fn handle_new_stream(&mut self, event: proto_msg::Event) -> Result<(), ServerError> {
        log::debug!("Handling `new_stream`");

        // Connecting new node
        let stream = self.stream_channel_rx.recv().unwrap();
        let addr = stream.peer_addr().unwrap();

        log::info!("New node connected {}", addr);

        let fd = stream.as_raw_fd();
        self.nodes.insert(fd, stream);
        {
            let mut nodes_ips = self.nodes_ips.lock().unwrap();
            nodes_ips.insert(addr.ip(), fd);
        }

        let bytes = event.data.get(0).unwrap();
        let node_id = utils::u128_from_ne_bytes(bytes).unwrap();

        self.nodes_ids.insert(node_id, fd);

        // Notify `node` about new connection
        self.main_event_channel_tx.send(proto_msg::Event {
            dir: Some(proto_msg::event::Dir::Incoming as i32),
            dest: Some(proto_msg::event::Dest::Node as i32),
            kind: proto_msg::event::Kind::NodeConnected as i32,
            data: event.data,
            meta: vec![]
        }).unwrap();

        // Notify `listener` thread about new client
        self.event_channel_tx.send(proto_msg::Event {
            dir: Some(proto_msg::event::Dir::Incoming as i32),
            dest: None,
            kind: proto_msg::event::Kind::NewFd as i32,
            data: vec![fd.to_ne_bytes().to_vec()],
            meta: vec![]
        }).unwrap();

        Ok(())
    }

    fn handle_broadcast_event(&mut self, event: proto_msg::Event) -> Result<(), ServerError> {
        let actual_event = event.data.get(0).unwrap();

        let bytes = event.data.get(1).unwrap();
        let nodes_count = utils::i32_from_ne_bytes(bytes).unwrap();

        for i in 0..nodes_count {
            let index = 2 + i;
            let bytes = event.data.get(index as usize).unwrap();
            let node_id = utils::u128_from_ne_bytes(bytes).unwrap();

            if let Some(fd) = self.nodes_ids.get(&node_id) {
                let mut stream = self.nodes.get(&fd).unwrap();
                stream.write(actual_event).unwrap();
            }
        }

        Ok(())
    }

    fn handle_approve_transaction(&mut self, event: proto_msg::Event) -> Result<(), ServerError> {
        log::debug!("Handling `approve_transaction`");

        let bytes = event.meta.get(0).unwrap();
        let node_fd = utils::i32_from_ne_bytes(bytes).unwrap();

        let meta = (&event.meta[1..]).to_vec();

        // TODO: Add check for node being already disconnected
        let mut node_stream = self.nodes.get(&node_fd).unwrap();

        // Removing meta information
        let event = proto_msg::Event {
            dir: event.dir,
            dest: event.dest,
            kind: event.kind,
            data: event.data,
            meta
        };

        node_stream.write(&event::serialize(event)).unwrap();

        Ok(())
    }

    fn handle_respond_client(&mut self, event: proto_msg::Event) -> Result<(), ServerError> {
        log::debug!("Handling `respond_client`");

        let first_arg = event.meta.get(0).unwrap();
        let client_fd = utils::i32_from_ne_bytes(first_arg).unwrap();

        let meta = (&event.meta[1..]).to_vec();

        if self.clients.contains_key(&client_fd) {
            let mut client_stream = self.clients.get(&client_fd).unwrap();

            // Removing meta information
            let event = proto_msg::Event {
                dir: event.dir,
                dest: event.dest,
                kind: event.kind,
                data: event.data,
                meta
            };

            client_stream.write(&event::serialize(event)).unwrap();
        }

        Ok(())
    }

    fn t_scanner(server_event_tx: mpsc::Sender<proto_msg::Event>,
				 server_stream_tx: mpsc::Sender<TcpStream>,
                 known_nodes_ips: Arc<Mutex<HashMap<IpAddr, i32>>>,
                 node_id: u128) {
        log::debug!("Scanner thread started");

        loop {
            let local_ips = utils::get_ipv4_ips();
            for (network, _mask) in utils::get_networks_and_masks().iter() {
                if !network.is_ipv4() {
                    continue;
                }

                let octets = utils::get_octets(network).unwrap();

                // Ping all avaliable IPs (not really, but OK for MVP)
                for i in 0..255 {
                    let ip = IpAddr::V4(Ipv4Addr::new(octets[0], octets[1], octets[2], i));
                    {
                        let known_nodes_ips = known_nodes_ips.lock().unwrap();
                        if local_ips.contains(&ip) || known_nodes_ips.contains_key(&ip) {
                            continue;
                        }
                    }

                    let socket_address = SocketAddr::new(ip, 32000);
                    let stream = TcpStream::connect_timeout(
                        &socket_address,
                        time::Duration::from_millis(100)
                    );

                    match stream {
                        Ok(mut stream) => {
                            // MarkMeNode
                            let event = proto_msg::Event {
                                dir: None,
                                dest: None,
                                kind: proto_msg::event::Kind::MarkMeNode as i32,
                                data: vec![node_id.to_ne_bytes().to_vec()],
                                meta: vec![]
                            };
                            stream.write(&event::serialize(event)).unwrap();

                            let events = utils::read_events(&mut stream).unwrap();
                            let event = events.get(0).unwrap();
                            let bytes = event.data.get(0).unwrap();
                            let node_id = utils::u128_from_ne_bytes(bytes).unwrap();

                            // Send stream of client that responded
                            server_stream_tx.send(stream).unwrap();
                            // Notify server, that new node detected
                            server_event_tx.send(proto_msg::Event {
                                dir: Some(proto_msg::event::Dir::Incoming as i32),
                                dest: None,
                                kind: proto_msg::event::Kind::NewStream as i32,
                                data: vec![node_id.to_ne_bytes().to_vec()],
                                meta: vec![]
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
}

impl From<FSMError> for ServerError {
    fn from(_: FSMError) -> Self {
        ServerError::InternalError
    }
}
