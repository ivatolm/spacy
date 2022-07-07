use std::{
    collections::HashMap,
    sync::mpsc,
    process::Command,
    net::{TcpListener, TcpStream}
};
use common::{
    fsm::{FSM, FSMError},
    event::proto_msg
};

pub struct PluginMan {
    fsm: FSM,
    event_channel_tx: mpsc::Sender<proto_msg::Event>,
    event_channel_rx: mpsc::Receiver<proto_msg::Event>,
    listener: TcpListener,
    plugins: HashMap<u32, TcpStream>,

    main_event_channel_tx: mpsc::Sender<proto_msg::Event>
}

#[derive(Debug)]
pub enum PluginManError {
    InternalError
}

impl PluginMan {
    // 0 - initialization
    // 1 - waiting for events
    // 2 - handle incoming event
    // 3 - handle outcoming event
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

        // Creating listener for communication with plugins
        let listener = TcpListener::bind(("127.0.0.1", 32002)).unwrap();

        Self {
            fsm,
            event_channel_tx,
            event_channel_rx,
            listener,
            plugins: HashMap::new(),

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
            2 => self.handle_incoming_event(),
            3 => self.handle_outcoming_event(),
            4 => {
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

        let event = match self.event_channel_rx.try_recv() {
            Ok(event) => event,
            Err(_) => {
                return Ok(());
            }
        };

        self.fsm.push_event(event);

        self.fsm.transition(2)?;
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

                    self.fsm.transition(1)?;
                    return Ok(());
                }
            };

            // TODO: somehow make this call non-blocking (timeout)
            let stream = match self.listener.accept() {
                Ok((stream, _addr)) => stream,
                Err(err) => {
                    log::warn!("Error occured while accepting plugin's connection: {}", err);
                    child.kill().unwrap();

                    self.fsm.transition(1)?;
                    return Ok(());
                }
            };

            self.plugins.insert(child.id(), stream);

            log::info!("New plugin started");
        }

        self.fsm.transition(1)?;
        Ok(())
    }

    fn handle_outcoming_event(&mut self) -> Result<(), PluginManError> {
        log::debug!("State `handle_outcoming_event`");

        let event = self.fsm.pop_event().unwrap();

        self.fsm.transition(1)?;
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
