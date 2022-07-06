use std::{
    collections::HashMap,
    sync::mpsc
};
use common::{
    fsm::{FSM, FSMError},
    event::proto_msg,
    utils
};

pub struct Node {
    fsm: FSM,
    event_channel_tx: mpsc::Sender<proto_msg::Event>,
    event_channel_rx: mpsc::Receiver<proto_msg::Event>,
    shared_memory: HashMap<u8, (u8, Vec<u8>)>,
    shared_memory_version: i32,

    main_event_channel_tx: mpsc::Sender<proto_msg::Event>
}

pub enum NodeError {
    InternalError
}

impl Node {
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

        // Creating a shared_memory instance
        let shared_memory = HashMap::new();
        let shared_memory_version = 0;

        Self {
            fsm,
            event_channel_tx,
            event_channel_rx,
            shared_memory,
            shared_memory_version,

            main_event_channel_tx
        }
    }

    pub fn start(&mut self) -> mpsc::Sender<proto_msg::Event> {
        let event_channel_tx_clone = self.event_channel_tx.clone();

        event_channel_tx_clone
    }

    pub fn step(&mut self) -> Result<(), NodeError> {
        match self.fsm.state {
            0 => self.init(),
            1 => self.wait_event(),
            2 => self.handle_incoming_event(),
            _ => unreachable!()
        }
    }

    fn init(&mut self) -> Result<(), NodeError> {
        log::debug!("State `init`");

        // Nothing to do

        self.fsm.transition(1)?;
        Ok(())
    }

    fn wait_event(&mut self) -> Result<(), NodeError> {
        log::debug!("State `wait_event`");

        let event = match self.event_channel_rx.recv() {
            Ok(event) => event,
            Err(err) => {
                log::error!("Error occured while receiving event: {}", err);
                panic!();
            }
        };

        let event_direction = event.dir;
        self.fsm.push_event(event);

        // Handling event based on it's direction
        if event_direction == proto_msg::event::Direction::Incoming as i32 {
            log::debug!("Received `incoming` event");

            self.fsm.transition(2)?;
        }

        Ok(())
    }

    fn handle_incoming_event(&mut self) -> Result<(), NodeError> {
        log::debug!("State `handle_incoming_event`");

        let event = self.fsm.pop_event().unwrap();

        // Receive new version and update
        if event.kind == proto_msg::event::Kind::UpdateSharedMemory as i32 {
            let bytes = event.data.get(0).unwrap();
            let version = utils::i32_from_ne_bytes(bytes).unwrap();

            if version > self.shared_memory_version {
                // Updating local shared memory
                log::debug!("Shared memory update contains {} diffs with local", event.data.len() / 3);
            }
        }

        Ok(())
    }

    fn handle_outcoming_event(&mut self) -> Result<(), NodeError> {
        log::debug!("State `handle_outcoming_event`");

        let event = self.fsm.pop_event().unwrap();

        // Update and send new version
        if event.kind == proto_msg::event::Kind::UpdateSharedMemory as i32 {
            log::debug!("Broadcasting shared memory update");
        }

        Ok(())
    }

    fn stop(self) -> Result<(), NodeError> {
        log::debug!("State `stop`");

        Ok(())
    }
}

impl From<FSMError> for NodeError {
    fn from(_: FSMError) -> Self {
        NodeError::InternalError
    }
}
