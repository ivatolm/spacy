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
                    let event_with_id = proto_msg::Event {
                        dir: event.dir,
                        dest: event.dest,
                        kind: event.kind,
                        data: event.data,
                        meta: vec![fd.to_ne_bytes().to_vec()]
                    };

                    self.fsm.push_event(event_with_id);
                }
            }
        }

        self.fsm.transition(2)?;
        Ok(())
    }

    fn handle_event(&mut self) -> Result<(), PluginManError> {
        log::debug!("State `handle_event`");

        let event = match self.fsm.pop_event() {
            Some(event) => event,
            None => {
                self.fsm.transition(1)?;
                return Ok(());
            }
        };
        let event_direction = event.dir;
        self.fsm.push_event(event);

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

        let event = self.fsm.pop_event().unwrap();

        if event.kind == proto_msg::event::Kind::NewPlugin as i32 {
            let first_arg = event.data.get(0).unwrap();
            let plugin_source = String::from_utf8_lossy(&first_arg).to_string();

            // Spawning new thread with the plugin
            let exec_status = Command::new("python3")
                .arg("-c")
                .arg(plugin_source)
                .spawn();

            let mut child = match exec_status {
                Ok(child) => child,
                Err(err) => {
                    log::warn!("Error occured while starting a plugin: {}", err);
                    // TODO: Notify client about failure

                    self.fsm.transition(2)?;
                    return Ok(());
                }
            };

            // TODO: somehow make this call non-blocking (timeout)
            let stream = match self.listener.accept() {
                Ok((stream, _addr)) => stream,
                Err(err) => {
                    log::warn!("Error occured while accepting plugin's connection: {}", err);
                    child.kill().unwrap();

                    self.fsm.transition(2)?;
                    return Ok(());
                }
            };

            self.plugins.insert(child.id(), stream.as_raw_fd());
            self.plugins_streams.insert(stream.as_raw_fd(), stream);

            log::info!("New plugin started");
        }

        else if event.kind == proto_msg::event::Kind::GetFromSharedMemory as i32 {
            // Sending an event to plugin
            let first_arg = event.meta.get(0).unwrap();
            let plugin_fd = utils::i32_from_ne_bytes(first_arg).unwrap();

            let stream = self.plugins_streams.get_mut(&plugin_fd).unwrap();

            let event = proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Incoming as i32),
                dest: None,
                kind: proto_msg::event::Kind::GetFromSharedMemory as i32,
                data: event.data,
                meta: vec![]
            };

            stream.write(&event::serialize(event)).unwrap();
        }

        else {
            log::warn!("Received event with unknown kind");
        }

        self.fsm.transition(2)?;
        Ok(())
    }

    fn handle_outcoming_event(&mut self) -> Result<(), PluginManError> {
        log::debug!("State `handle_outcoming_event`");

        let event = self.fsm.pop_event().unwrap();
        let mut event_to_main = None;

        if event.kind == proto_msg::event::Kind::UpdateSharedMemory as i32 {
            log::debug!("Plugin requested an update of shared memory");

            event_to_main = Some(proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Outcoming as i32),
                dest: Some(proto_msg::event::Dest::Node as i32),
                kind: event.kind,
                data: event.data,
                meta: event.meta
            });
        }

        else if event.kind == proto_msg::event::Kind::GetFromSharedMemory as i32 {
            log::debug!("Plugin requested a value from shared memory");

            event_to_main = Some(proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Outcoming as i32),
                dest: Some(proto_msg::event::Dest::Node as i32),
                kind: event.kind,
                data: event.data,
                meta: event.meta
            });
        }

        else {
            log::warn!("Received event with unknown kind");
        }

        if let Some(event) = event_to_main {
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
