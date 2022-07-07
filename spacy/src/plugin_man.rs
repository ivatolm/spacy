use std::{
    collections::HashMap,
    sync::mpsc
};
use common::{
    fsm::{FSM, FSMError},
    event::proto_msg
};

pub struct PluginMan {
    fsm: FSM,
    event_channel_tx: mpsc::Sender<proto_msg::Event>,
    event_channel_rx: mpsc::Receiver<proto_msg::Event>,

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

        Self {
            fsm,
            event_channel_tx,
            event_channel_rx,

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

        // Nothing to do

        self.fsm.transition(1)?;
        Ok(())
    }

    fn wait_event(&mut self) -> Result<(), PluginManError> {
        log::debug!("State `wait_event`");

        let event = match self.event_channel_rx.recv() {
            Ok(event) => event,
            Err(err) => {
                log::error!("Error occured while receiving event: {}", err);
                panic!();
            }
        };

        self.fsm.push_event(event);

        self.fsm.transition(2)?;
        Ok(())
    }

    fn handle_incoming_event(&mut self) -> Result<(), PluginManError> {
        log::debug!("State `handle_incoming_event`");

        let event = self.fsm.pop_event().unwrap();

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
