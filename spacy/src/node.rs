use std::{
    collections::HashMap,
    sync::mpsc, time
};
use common::{
    fsm::{FSM, FSMError},
    event::{proto_msg, self},
    utils
};

pub struct Node {
    fsm: FSM,
    event_channel_tx: mpsc::Sender<proto_msg::Event>,
    event_channel_rx: mpsc::Receiver<proto_msg::Event>,
    shared_memory: HashMap<i32, Vec<u8>>,
    shared_memory_version: u128,

    main_event_channel_tx: mpsc::Sender<proto_msg::Event>
}

#[derive(Debug)]
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
            3 => self.handle_outcoming_event(),
            4 => {
                self.stop()?;
                return Ok(());
            },
            _ => unreachable!()
        }
    }

    fn init(&mut self) -> Result<(), NodeError> {
        log::debug!("State `init`");

        self.fsm.transition(1)?;
        Ok(())
    }

    fn wait_event(&mut self) -> Result<(), NodeError> {
        log::debug!("State `wait_event`");

        let event = match self.event_channel_rx.try_recv() {
            Ok(event) => event,
            Err(_) => {
                return Ok(());
            }
        };

        let event_direction = event.dir;
        self.fsm.push_event(event);

        // Handling event based on it's direction
        if let Some(dir) = event_direction {
            if dir == proto_msg::event::Dir::Incoming as i32 {
                log::debug!("Received `incoming` event");

                self.fsm.transition(2)?;
            }

            else if dir == proto_msg::event::Dir::Outcoming as i32 {
                log::debug!("Received `outcoming` event");

                self.fsm.transition(3)?;
            }

            else {
                log::warn!("Received event with the unknown direction");
            }

        } else {
            log::warn!("Received event without direction");
        }

        Ok(())
    }

    fn handle_incoming_event(&mut self) -> Result<(), NodeError> {
        log::debug!("State `handle_incoming_event`");

        let event = self.fsm.pop_event().unwrap();

        // Receive new version and update
        if event.kind == proto_msg::event::Kind::UpdateSharedMemory as i32 {
            log::debug!("Handling `update_shared_memory`");

            let version_bytes = event.data.get(0).unwrap();
            let key_bytes = event.data.get(1).unwrap();
            let value_bytes = event.data.get(2).unwrap();

            // Parsing update's data
            let version = utils::u128_from_ne_bytes(version_bytes).unwrap();
            let key = utils::i32_from_ne_bytes(key_bytes).unwrap();
            let value = value_bytes.to_vec();

            // If received version is actually newer than local
            if version > self.shared_memory_version {
                // Updating local shared memory
                self.shared_memory.insert(key, value);
                self.shared_memory_version = version;
            }
        }

        else {
            log::warn!("Received event with unknown kind: {}", event.kind);
        }

        self.fsm.transition(1)?;
        Ok(())
    }

    fn handle_outcoming_event(&mut self) -> Result<(), NodeError> {
        log::debug!("State `handle_outcoming_event`");

        let event = self.fsm.pop_event().unwrap();
        let mut events_to_main = vec![];

        // Update and send new version
        if event.kind == proto_msg::event::Kind::UpdateSharedMemory as i32 {
            log::debug!("Handling `update_shared_memory`");

            // Parsing received update
            let version = time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_nanos();
            let key = utils::i32_from_ne_bytes(event.data.get(0).unwrap()).unwrap();
            let value = event.data.get(1).unwrap().to_vec();

            // Updating local shared memory
            self.shared_memory.insert(key, value.clone());
            self.shared_memory_version = version;

            // Creating update event
            let actual_event = proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Incoming as i32),
                dest: None,
                kind: proto_msg::event::Kind::UpdateSharedMemory as i32,
                data: vec![version.to_ne_bytes().to_vec(), key.to_ne_bytes().to_vec(), value],
                meta: vec![]
            };

            // Propagating update event to other nodes
            let broadcast_event = proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Outcoming as i32),
                dest: Some(proto_msg::event::Dest::Server as i32),
                kind: proto_msg::event::Kind::BroadcastEvent as i32,
                data: vec![event::serialize(actual_event)],
                meta: vec![]
            };

            events_to_main.push(broadcast_event);
        }

        else if event.kind == proto_msg::event::Kind::GetFromSharedMemory as i32 {
            log::debug!("Returning value from shared memory");

            let first_arg = event.data.get(0).unwrap();

            // Parsing event
            let key = utils::i32_from_ne_bytes(first_arg).unwrap();

            // Returning value if key is valid
            let data;
            if self.shared_memory.contains_key(&key) {
                let value = self.shared_memory.get(&key).unwrap();
                data = vec![value.to_vec()];
            } else {
                data = vec![];
            }

            // Creating event for response
            let event = proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Incoming as i32),
                dest: Some(proto_msg::event::Dest::PluginMan as i32),
                kind: proto_msg::event::Kind::GetFromSharedMemory as i32,
                data,
                meta: event.meta
            };

            events_to_main.push(event);
        }

        else {
            log::warn!("Received event with unknown kind: {}", event.kind);
        }

        // Send events to main
        for event in events_to_main {
            self.main_event_channel_tx.send(event).unwrap();
        }

        self.fsm.transition(1)?;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), NodeError> {
        log::debug!("State `stop`");

        Ok(())
    }
}

impl From<FSMError> for NodeError {
    fn from(_: FSMError) -> Self {
        NodeError::InternalError
    }
}
