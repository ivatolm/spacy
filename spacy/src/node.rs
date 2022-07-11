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
    node_id: u128,
    nodes_count: usize,

    in_transaction: bool,
    transaction_event: Option<proto_msg::Event>,
    transaction_approvals: usize,

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

        // Generating node id
        let node_id = time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_nanos();

        Self {
            fsm,
            event_channel_tx,
            event_channel_rx,
            shared_memory,
            shared_memory_version,
            node_id,
            nodes_count: 0,

            in_transaction: false,
            transaction_event: None,
            transaction_approvals: 0,

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

        let event = self.fsm.pop_front_event().unwrap();

        if event.kind == proto_msg::event::Kind::UpdateNodeCount as i32 {
            self.handle_update_node_count(event)?;
        }

        else if event.kind == proto_msg::event::Kind::RequestTransaction as i32 {
            self.handle_request_transaction_incoming(event)?;
        }

        else if event.kind == proto_msg::event::Kind::ApproveTransaction as i32 {
            self.handle_approve_transaction()?;
        }

        else if event.kind == proto_msg::event::Kind::CommitTransaction as i32 {
            self.handle_perform_transaction(event)?;
        }

        else {
            log::warn!("Received event with unknown kind: {}", event.kind);
        }

        self.fsm.transition(1)?;
        Ok(())
    }

    fn handle_outcoming_event(&mut self) -> Result<(), NodeError> {
        log::debug!("State `handle_outcoming_event`");

        let event = self.fsm.pop_front_event().unwrap();

        if event.kind == proto_msg::event::Kind::UpdateSharedMemory as i32 {
            self.handle_update_shared_memory_outcoming(event)?;
        }

        else if event.kind == proto_msg::event::Kind::GetFromSharedMemory as i32 {
            self.handle_get_from_shared_memory(event)?;
        }

        else {
            log::warn!("Received event with unknown kind: {}", event.kind);
        }

        self.fsm.transition(1)?;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), NodeError> {
        log::debug!("State `stop`");

        Ok(())
    }

    fn handle_update_node_count(&mut self, event: proto_msg::Event) -> Result<(), NodeError> {
        log::debug!("Handling `update_node_count`");

        let bytes = event.data.get(0).unwrap();
        self.nodes_count = utils::usize_from_ne_bytes(bytes).unwrap();

        Ok(())
    }

    fn handle_update_shared_memory_outcoming(&mut self, event: proto_msg::Event) -> Result<(), NodeError> {
        log::debug!("Handling `update_shared_memory`");

        // Parsing received update
        let version = time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_nanos();
        let key = utils::i32_from_ne_bytes(event.data.get(0).unwrap()).unwrap();
        let value = event.data.get(1).unwrap().to_vec();

        if self.in_transaction {
            log::debug!("Transaction failed: already in transaction");

            let plugin_response_event = proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Incoming as i32),
                dest: Some(proto_msg::event::Dest::PluginMan as i32),
                kind: proto_msg::event::Kind::TransactionFailed as i32,
                data: vec![],
                meta: event.meta
            };

            self.main_event_channel_tx.send(plugin_response_event).unwrap();

            return Ok(());
        }

        else {
            self.in_transaction = true;
            self.transaction_event = Some(proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Incoming as i32),
                dest: None,
                kind: proto_msg::event::Kind::CommitTransaction as i32,
                data: vec![version.to_ne_bytes().to_vec(), key.to_ne_bytes().to_vec(), value],
                meta: vec![]
            });

            // Requesting transaction
            self.handle_request_transaction_outcoming()?;
        }

        Ok(())
    }

    fn handle_get_from_shared_memory(&mut self, event: proto_msg::Event) -> Result<(), NodeError> {
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

        self.main_event_channel_tx.send(event).unwrap();

        Ok(())
    }

    fn handle_request_transaction_incoming(&mut self, event: proto_msg::Event) -> Result<(), NodeError> {
        log::debug!("Handling `request_transaction`");

        // Transaction collision
        if self.in_transaction {
            let bytes = event.meta.get(1).unwrap();
            let node_id = utils::u128_from_ne_bytes(bytes).unwrap();

            // Other node will perform
            if node_id > self.node_id {
                log::debug!("Local transaction was dropped");

                let meta = event.meta.get(0).unwrap();
                let node_response_event = proto_msg::Event {
                    dir: Some(proto_msg::event::Dir::Outcoming as i32),
                    dest: Some(proto_msg::event::Dest::Server as i32),
                    kind: proto_msg::event::Kind::ApproveTransaction as i32,
                    data: vec![],
                    meta: vec![meta.to_vec()]
                };

                self.main_event_channel_tx.send(node_response_event).unwrap();

                // Notifing plugin about failed transaction
                let transaction_event = self.transaction_event.as_ref().unwrap();
                let meta = transaction_event.meta.get(0).unwrap();

                let plugin_response_event = proto_msg::Event {
                    dir: Some(proto_msg::event::Dir::Incoming as i32),
                    dest: Some(proto_msg::event::Dest::PluginMan as i32),
                    kind: proto_msg::event::Kind::TransactionFailed as i32,
                    data: vec![],
                    meta: vec![meta.to_vec()]
                };

                self.main_event_channel_tx.send(plugin_response_event).unwrap();

                self.in_transaction = false;
                self.transaction_event = None;
            }
        }

        else {
            self.in_transaction = true;

            let meta = event.meta.get(0).unwrap();
            let node_response_event = proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Outcoming as i32),
                dest: Some(proto_msg::event::Dest::Server as i32),
                kind: proto_msg::event::Kind::ApproveTransaction as i32,
                data: vec![],
                meta: vec![meta.to_vec()]
            };

            self.main_event_channel_tx.send(node_response_event).unwrap();
        }

        Ok(())
    }

    fn handle_request_transaction_outcoming(&mut self) -> Result<(), NodeError> {
        log::debug!("Handling `request_transaction`");

        // Requesting transaction event
        let actual_event = proto_msg::Event {
            dir: Some(proto_msg::event::Dir::Incoming as i32),
            dest: None,
            kind: proto_msg::event::Kind::RequestTransaction as i32,
            data: vec![],
            meta: vec![]
        };

        // Propagating to other nodes
        let broadcast_event = proto_msg::Event {
            dir: Some(proto_msg::event::Dir::Outcoming as i32),
            dest: Some(proto_msg::event::Dest::Server as i32),
            kind: proto_msg::event::Kind::BroadcastEvent as i32,
            data: vec![event::serialize(actual_event)],
            meta: vec![self.node_id.to_ne_bytes().to_vec()]
        };

        self.main_event_channel_tx.send(broadcast_event).unwrap();

        Ok(())
    }

    fn handle_approve_transaction(&mut self) -> Result<(), NodeError> {
        log::debug!("Handling `approve_transaction`");

        self.transaction_approvals += 1;
        if self.transaction_approvals >= self.nodes_count {
            let transaction_event = self.transaction_event.clone();

            let transaction_event = transaction_event.as_ref().unwrap();
            self.handle_perform_transaction(transaction_event.clone())?;
            self.transaction_approvals = 0;

            // Propagating to other nodes
            let broadcast_event = proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Outcoming as i32),
                dest: Some(proto_msg::event::Dest::Server as i32),
                kind: proto_msg::event::Kind::BroadcastEvent as i32,
                data: vec![event::serialize(transaction_event.clone())],
                meta: vec![self.node_id.to_ne_bytes().to_vec()]
            };

            self.main_event_channel_tx.send(broadcast_event).unwrap();
        }

        Ok(())
    }

    fn handle_perform_transaction(&mut self, event: proto_msg::Event) -> Result<(), NodeError> {
        log::debug!("Handling `perform_transaction`");

        // Parsing received update
        let version_bytes = event.data.get(0).unwrap();
        let key_bytes = event.data.get(1).unwrap();
        let value_bytes = event.data.get(2).unwrap();

        // Parsing update's data
        let version = utils::u128_from_ne_bytes(version_bytes).unwrap();
        let key = utils::i32_from_ne_bytes(key_bytes).unwrap();
        let value = value_bytes.to_vec();

        // Updating local shared memory
        self.shared_memory.insert(key, value.clone());
        self.shared_memory_version = version;

        self.in_transaction = false;
        self.transaction_event = None;

        Ok(())
    }
}

impl From<FSMError> for NodeError {
    fn from(_: FSMError) -> Self {
        NodeError::InternalError
    }
}
