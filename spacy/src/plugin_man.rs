use std::{
    collections::HashMap,
    sync::mpsc,
    process::Command,
    net::{TcpListener, TcpStream, Shutdown},
    os::unix::prelude::AsRawFd,
    io::Write
};
use common::{
    fsm::{FSM, FSMError},
    event::{proto_msg, self}, utils
};
use nix::sys::{
    select::{select, FdSet},
    time::{TimeVal, TimeValLike}
};

pub struct PluginMan {
    fsm: FSM,
    event_channel_tx: mpsc::Sender<proto_msg::Event>,
    event_channel_rx: mpsc::Receiver<proto_msg::Event>,
    listener: TcpListener,
    plugins: HashMap<u32, i32>,
    plugins_names: HashMap<Vec<u8>, u32>,
    plugins_streams: HashMap<i32, TcpStream>,

    main_event_channel_tx: mpsc::Sender<proto_msg::Event>
}

#[derive(Debug)]
pub enum PluginManError {
    InternalError
}

impl PluginMan {
    // 0 - initialization
    // 1 - waiting for events
    // 2 - handle event
    // 3 - handle incoming event
    // 4 - handle outcoming event
    // 5 - stop

    pub fn new(main_event_channel_tx: mpsc::Sender<proto_msg::Event>) -> Self {
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

        // Creating listener for communication with plugins
        let listener = TcpListener::bind(("127.0.0.1", 32002)).unwrap();

        Self {
            fsm,
            event_channel_tx,
            event_channel_rx,
            listener,
            plugins: HashMap::new(),
            plugins_names: HashMap::new(),
            plugins_streams: HashMap::new(),

            main_event_channel_tx
        }
    }

    pub fn start(&mut self) -> mpsc::Sender<proto_msg::Event> {
        let event_channel_tx_clone = self.event_channel_tx.clone();

        event_channel_tx_clone
    }

    pub fn step(&mut self) -> Result<(), PluginManError> {
        match self.fsm.state {
            0 => self.init(),
            1 => self.wait_event(),
            2 => self.handle_event(),
            3 => self.handle_incoming_event(),
            4 => self.handle_outcoming_event(),
            5 => {
                self.stop()?;
                return Ok(());
            }
            _ => unreachable!()
        }
    }

    fn init(&mut self) -> Result<(), PluginManError> {
        log::debug!("State `init`");

        self.fsm.transition(1)?;
        Ok(())
    }

    fn wait_event(&mut self) -> Result<(), PluginManError> {
        log::debug!("State `wait_event`");

        // Checking if there anything to read
        match self.event_channel_rx.try_recv() {
            Ok(event) => {
                self.fsm.push_event(event);
            },
            Err(_) => {}
        }

        let mut readfds = FdSet::new();
        for fd in self.plugins.values() {
            readfds.insert(*fd);
        }

        let mut timeout = TimeVal::milliseconds(10);
        let result = select(None, &mut readfds, None, None, &mut timeout);
        if result.is_err() {
            return Ok(());
        }

        for fd in readfds.fds(None) {
            let mut stream = self.plugins_streams.get_mut(&fd).unwrap();

            // Reading message until read
            let message = utils::read_full_stream(&mut stream).unwrap();

            // If plugin manager disconnected
            if message.len() == 0 {
                stream.shutdown(Shutdown::Both).unwrap();
            } else {
                let events = event::deserialize(&message).unwrap();
                for event in events {
                    // Adding plugin's id to event's meta information
                    let mut event_meta = event.meta;
                    event_meta.insert(0, fd.to_ne_bytes().to_vec());

                    let event_with_meta = proto_msg::Event {
                        dir: event.dir,
                        dest: event.dest,
                        kind: event.kind,
                        data: event.data,
                        meta: event_meta
                    };

                    self.fsm.push_front_event(event_with_meta);
                }
            }
        }

        self.fsm.transition(2)?;
        Ok(())
    }

    fn handle_event(&mut self) -> Result<(), PluginManError> {
        log::debug!("State `handle_event`");

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

    fn handle_incoming_event(&mut self) -> Result<(), PluginManError> {
        log::debug!("State `handle_incoming_event`");

        let event = self.fsm.pop_front_event().unwrap();
        let mut events_to_main = vec![];

        if event.kind == proto_msg::event::Kind::NewPlugin as i32 {
            log::debug!("Handling `new_plugin`");
            // Startup status variable
            let mut status: i32 = 0;

            let first_arg = event.data.get(0).unwrap();
            let second_arg = event.data.get(1).unwrap();

            // Parsing event data
            let plugin_source = String::from_utf8_lossy(&first_arg).to_string();
            let plugin_name = second_arg.to_vec();

            // Spawning new thread with the plugin
            let exec_status = Command::new("python3")
                .arg("-c")
                .arg(plugin_source)
                .spawn();

            // Check if child is started successfully
            match exec_status {
                Ok(mut child) => {
                    // Accept plugin's connection
                    // TODO: somehow make this call non-blocking (timeout)
                    match self.listener.accept() {
                        Ok((stream, _addr)) => {
                            // Add plugin to local structs
                            let child_id = child.id();
                            self.plugins.insert(child_id, stream.as_raw_fd());
                            self.plugins_names.insert(plugin_name, child_id);
                            self.plugins_streams.insert(stream.as_raw_fd(), stream);

                            log::info!("New plugin started");
                        },
                        Err(err) => {
                            log::warn!("Error occured while accepting plugin's connection: {}", err);
                            child.kill().unwrap();
                            status = -2;
                        }
                    };
                },
                Err(err) => {
                    log::warn!("Error occured while starting a plugin: {}", err);
                    status = -1;
                }
            }

            // TODO: Notify client about status
            let response_event = proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Outcoming as i32),
                dest: Some(proto_msg::event::Dest::Server as i32),
                kind: proto_msg::event::Kind::RespondClient as i32,
                data: vec![status.to_ne_bytes().to_vec()],
                meta: event.meta
            };
            events_to_main.push(response_event);
        }

        else if event.kind == proto_msg::event::Kind::NewPluginEvent as i32 {
            log::debug!("Handling `new_plugin_event`");

            let first_arg = event.data.get(0).unwrap();
            let second_arg = event.data.get(1).unwrap();

            // Parsing event data
            let plugin_name = first_arg.to_vec();
            let events_to_plugin = event::deserialize(second_arg).unwrap();

            // Getting plugin's stream from the name
            let child_id = self.plugins_names.get(&plugin_name).unwrap();
            let fd = self.plugins.get(&child_id).unwrap();
            let mut stream = self.plugins_streams.get(&fd).unwrap();

            // Sending an event to plugin
            let meta = event.meta;
            for event in events_to_plugin {
                let meta_clone = meta.clone();
                let event = proto_msg::Event {
                    dir: Some(proto_msg::event::Dir::Incoming as i32),
                    dest: None,
                    kind: event.kind,
                    data: event.data,
                    meta: meta_clone
                };

                stream.write(&event::serialize(event)).unwrap();
            }
        }

        else if event.kind == proto_msg::event::Kind::GetFromSharedMemory as i32 {
            log::debug!("Handling `get_from_shared_memory`");

            // Parsing event data
            let first_arg = event.meta.get(0).unwrap();
            let plugin_fd = utils::i32_from_ne_bytes(first_arg).unwrap();

            // Getting plugin's stream
            let stream = self.plugins_streams.get_mut(&plugin_fd).unwrap();

            let event = proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Incoming as i32),
                dest: None,
                kind: proto_msg::event::Kind::GetFromSharedMemory as i32,
                data: event.data,
                meta: vec![]
            };

            // Sending an event to the plugin
            stream.write(&event::serialize(event)).unwrap();
        }

        else {
            log::warn!("Received event with unknown kind: {}", event.kind);
        }

        // Sending events to main
        for event in events_to_main {
            self.main_event_channel_tx.send(event).unwrap();
        }

        self.fsm.transition(2)?;
        Ok(())
    }

    fn handle_outcoming_event(&mut self) -> Result<(), PluginManError> {
        log::debug!("State `handle_outcoming_event`");

        let event = self.fsm.pop_front_event().unwrap();
        let mut events_to_main = vec![];

        if event.kind == proto_msg::event::Kind::UpdateSharedMemory as i32 {
            log::debug!("Handling `update_shared_memory`");

            let event = proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Outcoming as i32),
                dest: Some(proto_msg::event::Dest::Node as i32),
                kind: event.kind,
                data: event.data,
                meta: event.meta
            };
            events_to_main.push(event);
        }

        else if event.kind == proto_msg::event::Kind::GetFromSharedMemory as i32 {
            log::debug!("Handling `get_from_shared_memory`");

            let event = proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Outcoming as i32),
                dest: Some(proto_msg::event::Dest::Node as i32),
                kind: event.kind,
                data: event.data,
                meta: event.meta
            };
            events_to_main.push(event);
        }

        else if event.kind == proto_msg::event::Kind::RespondClient as i32 {
            log::debug!("Handling `respond_client`");

            // Removing unnecessary meta information
            let event_meta = &event.meta[1..];

            let event = proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Outcoming as i32),
                dest: Some(proto_msg::event::Dest::Server as i32),
                kind: event.kind,
                data: event.data,
                meta: event_meta.to_vec()
            };
            events_to_main.push(event);
        }

        else {
            log::warn!("Received event with unknown kind: {}", event.kind);
        }

        // Sending events to main
        for event in events_to_main {
            self.main_event_channel_tx.send(event).unwrap();
        }

        self.fsm.transition(2)?;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PluginManError> {
        log::debug!("State `stop`");

        Ok(())
    }
}

impl From<FSMError> for PluginManError {
    fn from(_: FSMError) -> Self {
        PluginManError::InternalError
    }
}
