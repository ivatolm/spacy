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
    nodes: Vec<u128>,

    is_transaction: bool,
    is_transaction_master: bool,
    transaction_kind: i32,
    transaction_queue: Vec<proto_msg::Event>,
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
        log::debug!("Node_id: {}", node_id);

        Self {
            fsm,
            event_channel_tx,
            event_channel_rx,
            shared_memory,
            shared_memory_version,

            node_id,
            nodes: vec![],

            is_transaction: false,
            is_transaction_master: false,
            transaction_kind: 0,
            transaction_queue: vec![],
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

    pub fn get_node_id(&mut self) -> u128 {
        self.node_id
    }

    fn init(&mut self) -> Result<(), NodeError> {
        log::debug!("State `init`");

        self.fsm.transition(1)?;
        Ok(())
    }

    fn wait_event(&mut self) -> Result<(), NodeError> {
        // log::debug!("State `wait_event`");

        let event = match self.event_channel_rx.try_recv() {
            Ok(event) => event,
            Err(_) => {
                if !self.is_transaction {
                    if self.fsm.is_queue_empty() {
                        if !self.transaction_queue.is_empty() {
                            let transaction_event = self.transaction_queue.get(0).unwrap();
                            let transaction_kind = transaction_event.kind;

                            self.handle_request_transaction_outcoming(transaction_kind, vec![])?;
                        }
                    }
                }

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

        if event.kind == proto_msg::event::Kind::RequestTransaction as i32 {
            self.handle_request_transaction_incoming(event)?;
        }

        else if event.kind == proto_msg::event::Kind::ApproveTransaction as i32 {
            self.handle_approve_transaction_incoming(event)?;
        }

        else if event.kind == proto_msg::event::Kind::CommitTransaction as i32 {
            self.handle_commit_transaction_incoming(event)?;
        }

        else if event.kind == proto_msg::event::Kind::NodeConnected as i32 {
            self.handle_request_new_connection_outcoming(event)?;
        }

        else if event.kind == proto_msg::event::Kind::NodeDisconnected as i32 {
            self.handle_request_old_connection_outcoming(event)?;
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
            self.handle_request_update_shared_memory_outcoming(event)?;
        }

        else if event.kind == proto_msg::event::Kind::GetFromSharedMemory as i32 {
            self.handle_request_get_from_shared_memory_outcoming(event)?;
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

    fn handle_request_transaction_incoming(&mut self, event: proto_msg::Event) -> Result<(), NodeError> {
        log::debug!("Handling `request_transaction`");

        let approve_event = proto_msg::Event {
            dir: Some(proto_msg::event::Dir::Outcoming as i32),
            dest: Some(proto_msg::event::Dest::Server as i32),
            kind: proto_msg::event::Kind::ApproveTransaction as i32,
            data: vec![self.node_id.to_ne_bytes().to_vec()],
            meta: event.meta
        };

        let bytes = event.data.get(0).unwrap();
        let transaction_kind = utils::i32_from_ne_bytes(bytes).unwrap();

        let bytes = event.data.get(1).unwrap();
        let node_id = utils::u128_from_ne_bytes(bytes).unwrap();

        if !self.nodes.contains(&node_id) {
            log::debug!("Ignoring request from non-connected node");
            return Ok(());
        }

        // If we are not in transaction
        if !self.is_transaction {
            self.is_transaction = true;
            self.transaction_kind = transaction_kind;

            // Approving transaction
            self.main_event_channel_tx.send(approve_event).unwrap();
        }

        // If we are in transaction, then collision occured
        else if self.is_transaction_master {
            if self.transaction_kind == transaction_kind {
                // If transaction is syncing shared memory
                if self.transaction_kind == 2 {
                    let bytes = event.data.get(2).unwrap();
                    let version = utils::u128_from_ne_bytes(bytes).unwrap();

                    if self.shared_memory_version == version {
                        if self.node_id > node_id {
                            log::debug!("Local transaction approved");
                        } else {
                            log::debug!("Local transaction cancelled");

                            self.transaction_queue.pop();

                            self.is_transaction_master = false;
                            self.transaction_approvals = 0;

                            // Approving transaction
                            self.main_event_channel_tx.send(approve_event).unwrap();
                        }
                    } else if self.shared_memory_version > version {
                        log::debug!("Local transaction approved");
                    } else {
                        log::debug!("Local transaction cancelled");

                        self.transaction_queue.pop();

                        self.is_transaction_master = false;
                        self.transaction_approvals = 0;

                        // Approving transaction
                        self.main_event_channel_tx.send(approve_event).unwrap();
                    }
                } else if self.transaction_kind == 3 {
                    if self.node_id > node_id {
                        log::debug!("Local transaction approved");
                    } else {
                        log::debug!("Local transaction cancelled");

                        let transaction_event = self.transaction_queue.pop().unwrap();
                        let meta = transaction_event.meta;

                        let event = proto_msg::Event {
                            dir: Some(proto_msg::event::Dir::Incoming as i32),
                            dest: Some(proto_msg::event::Dest::PluginMan as i32),
                            kind: proto_msg::event::Kind::TransactionFailed as i32,
                            data: vec![],
                            meta
                        };

                        self.main_event_channel_tx.send(event).unwrap();

                        self.is_transaction_master = false;
                        self.transaction_approvals = 0;

                        // Approving transaction
                        self.main_event_channel_tx.send(approve_event).unwrap();
                    }
                } else {
                    if self.node_id > node_id {
                        log::debug!("Local transaction approved");

                    } else {
                        log::debug!("Local transaction held");

                        self.is_transaction_master = false;
                        self.transaction_approvals = 0;

                        // Approving transaction
                        self.main_event_channel_tx.send(approve_event).unwrap();
                    }
                }
            } else if self.transaction_kind < transaction_kind {
                log::debug!("Local transaction approved");
            } else {
                if self.transaction_kind == 3 {
                    log::debug!("Local transaction failed");

                    let transaction_event = self.transaction_queue.pop().unwrap();
                    let meta = transaction_event.meta;

                    self.is_transaction_master = false;

                    let event = proto_msg::Event {
                        dir: Some(proto_msg::event::Dir::Incoming as i32),
                        dest: Some(proto_msg::event::Dest::PluginMan as i32),
                        kind: proto_msg::event::Kind::TransactionFailed as i32,
                        data: vec![],
                        meta
                    };

                    self.main_event_channel_tx.send(event).unwrap();
                } else {
                    log::debug!("Local transaction held");

                    self.is_transaction_master = false;
                    self.transaction_kind = transaction_kind;

                    // Approving transaction
                    self.main_event_channel_tx.send(approve_event).unwrap();
                }
            }
        }

        else {
            self.transaction_kind = transaction_kind;

            // Approving transaction
            self.main_event_channel_tx.send(approve_event).unwrap();
        }

        Ok(())
    }

    fn handle_request_transaction_outcoming(&mut self, transaction_kind: i32, additional_data: Vec<Vec<u8>>) -> Result<(), NodeError> {
        log::debug!("Handling `request_transaction`");

        // Requesting transaction event
        let mut data = additional_data;
        data.insert(0, transaction_kind.to_ne_bytes().to_vec());
        data.insert(1, self.node_id.to_ne_bytes().to_vec());

        let request_event = proto_msg::Event {
            dir: Some(proto_msg::event::Dir::Incoming as i32),
            dest: None,
            kind: proto_msg::event::Kind::RequestTransaction as i32,
            data,
            meta: vec![]
        };

        // Propagating to other nodes
        let mut data = vec![];
        data.push(event::serialize(request_event));
        data.push((self.nodes.len() as i32).to_ne_bytes().to_vec());
        for node in self.nodes.iter() {
            data.push(node.to_ne_bytes().to_vec());
        }

        let broadcast_event = proto_msg::Event {
            dir: Some(proto_msg::event::Dir::Outcoming as i32),
            dest: Some(proto_msg::event::Dest::Server as i32),
            kind: proto_msg::event::Kind::BroadcastEvent as i32,
            data,
            meta: vec![]
        };

        self.main_event_channel_tx.send(broadcast_event).unwrap();

        self.is_transaction = true;
        self.is_transaction_master = true;
        self.transaction_kind = transaction_kind;

        if self.nodes.len() == 0 {
            self.handle_try_perform_transaction()?;
        }

        Ok(())
    }

    fn handle_approve_transaction_incoming(&mut self, event: proto_msg::Event) -> Result<(), NodeError> {
        log::debug!("Handling `approve_transaction`");

        let bytes = event.data.get(0).unwrap();
        let node_id = utils::u128_from_ne_bytes(bytes).unwrap();

        if !self.nodes.contains(&node_id) {
            log::debug!("Ignoring request from non-connected node");
            return Ok(());
        }

        if self.is_transaction_master {
            self.transaction_approvals += 1;

            log::debug!("Total transaction approvals: {}/{}", self.transaction_approvals, self.nodes.len());

            self.handle_try_perform_transaction()?;
        }

        Ok(())
    }

    fn handle_commit_transaction_incoming(&mut self, event: proto_msg::Event) -> Result<(), NodeError> {
        log::debug!("Handling `commit_transaction`");

        let bytes = event.data.get(0).unwrap();
        let node_id = utils::u128_from_ne_bytes(bytes).unwrap();

        if !self.nodes.contains(&node_id) {
            log::debug!("Ignoring request from non-connected node");
            return Ok(());
        }

        self.handle_perform_transaction(event)?;

        Ok(())
    }

    fn handle_commit_transaction_outcoming(&mut self, event: proto_msg::Event) -> Result<(), NodeError> {
        log::debug!("Handling `commit_transaction`");

        let commit_event = proto_msg::Event {
            dir: Some(proto_msg::event::Dir::Incoming as i32),
            dest: None,
            kind: proto_msg::event::Kind::CommitTransaction as i32,
            data: event.data,
            meta: vec![]
        };

        let mut data = vec![];
        data.push(event::serialize(commit_event));
        data.push((self.nodes.len() as i32).to_ne_bytes().to_vec());
        for node in self.nodes.iter() {
            data.push(node.to_ne_bytes().to_vec());
        }

        let broadcast_event = proto_msg::Event {
            dir: Some(proto_msg::event::Dir::Outcoming as i32),
            dest: Some(proto_msg::event::Dest::Server as i32),
            kind: proto_msg::event::Kind::BroadcastEvent as i32,
            data,
            meta: vec![]
        };

        self.main_event_channel_tx.send(broadcast_event).unwrap();

        Ok(())
    }

    fn handle_request_new_connection_outcoming(&mut self, event: proto_msg::Event) -> Result<(), NodeError> {
        log::debug!("Handling `request new_connection`");

        let transaction_event = proto_msg::Event {
            dir: None,
            dest: None,
            kind: 0,
            data: event.data,
            meta: vec![]
        };

        self.transaction_queue.push(transaction_event);

        // Requesting transaction
        self.handle_request_transaction_outcoming(0, vec![])?;

        Ok(())
    }

    fn handle_request_old_connection_outcoming(&mut self, event: proto_msg::Event) -> Result<(), NodeError> {
        log::debug!("Handling `request old_connection`");

        let bytes = event.data.get(0).unwrap();
        let node_id = utils::u128_from_ne_bytes(bytes).unwrap();
        let mut to_del = None;
        for i in 0..self.nodes.len() {
            let id = self.nodes.get(i).unwrap();

            if node_id == *id {
                to_del = Some(i);
            }
        }

        if let Some(id) = to_del {
            self.nodes.remove(id);
        }

        // self.nodes.remove()

        let transaction_event = proto_msg::Event {
            dir: None,
            dest: None,
            kind: 1,
            data: event.data,
            meta: vec![]
        };

        self.transaction_queue.push(transaction_event);

        // Requesting transaction
        self.handle_request_transaction_outcoming(1, vec![])?;

        Ok(())
    }

    fn handle_request_update_shared_memory_outcoming(&mut self, event: proto_msg::Event) -> Result<(), NodeError> {
        log::debug!("Handling `request update_shared_memory`");

        if self.is_transaction {
            let event = proto_msg::Event {
                dir: Some(proto_msg::event::Dir::Incoming as i32),
                dest: Some(proto_msg::event::Dest::PluginMan as i32),
                kind: proto_msg::event::Kind::TransactionFailed as i32,
                data: vec![],
                meta: event.meta
            };

            self.main_event_channel_tx.send(event).unwrap();

            return Ok(());
        }

        let version = time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_nanos();
        let key = utils::i32_from_ne_bytes(event.data.get(0).unwrap()).unwrap();
        let value = event.data.get(1).unwrap().to_vec();

        let mut data = vec![];
        data.push(version.to_ne_bytes().to_vec());
        data.push(key.to_ne_bytes().to_vec());
        data.push(value);

        let transaction_event = proto_msg::Event {
            dir: None,
            dest: None,
            kind: 3,
            data,
            meta: event.meta
        };

        self.transaction_queue.push(transaction_event);

        // Requesting transaction
        self.handle_request_transaction_outcoming(3, vec![])?;

        Ok(())
    }

    fn handle_request_get_from_shared_memory_outcoming(&mut self, event: proto_msg::Event) -> Result<(), NodeError> {
        log::debug!("Handling `request get_from_shared_memory`");

        let bytes = event.data.get(0).unwrap();
        let key = utils::i32_from_ne_bytes(bytes).unwrap();

        // Returning value if key is valid
        let data;
        if self.shared_memory.contains_key(&key) {
            let value = self.shared_memory.get(&key).unwrap();
            data = vec![value.to_vec()];
        } else {
            data = vec![];
        }

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

    fn handle_try_perform_transaction(&mut self) -> Result<(), NodeError> {
        log::debug!("Handling `try_perform_transaction`");

        if self.transaction_approvals == self.nodes.len() {
            log::debug!("Transaction approved by all nodes");

            let mut transaction_event = self.transaction_queue.pop().unwrap();
            transaction_event.data.insert(0, self.node_id.to_ne_bytes().to_vec());
            transaction_event.data.insert(1, self.transaction_kind.to_ne_bytes().to_vec());

            let transaction_event_clone = transaction_event.clone();

            // committing transaction
            self.handle_commit_transaction_outcoming(transaction_event_clone.clone())?;

            // performing transaction
            self.handle_perform_transaction(transaction_event_clone)?;

            self.transaction_approvals = 0;
        }

        Ok(())
    }

    fn handle_perform_transaction(&mut self, event: proto_msg::Event) -> Result<(), NodeError> {
        log::debug!("Handling `perform_transaction`");

        // Unused
        // let bytes = event.data.get(0).unwrap();
        // let node_id = utils::u128_from_ne_bytes(bytes).unwrap();

        let bytes = event.data.get(1).unwrap();
        let transaction_kind = utils::i32_from_ne_bytes(bytes).unwrap();

        // Node connected
        if transaction_kind == 0 {
            log::debug!("Transaction kind: `node_connected`");

            let bytes = event.data.get(2).unwrap();
            let node_id = utils::u128_from_ne_bytes(bytes).unwrap();

            if node_id != self.node_id {
                self.nodes.push(node_id);
            }

            log::debug!("Node id: {}", node_id);

            self.is_transaction = false;
            self.is_transaction_master = false;

            // Syncing shared memory
            let mut data = vec![];
            data.push(self.shared_memory_version.to_ne_bytes().to_vec());
            for (key, value) in self.shared_memory.iter() {
                data.push(key.to_ne_bytes().to_vec());
                data.push(value.to_vec());
            }

            let transaction_event = proto_msg::Event {
                dir: None,
                dest: None,
                kind: 2,
                data,
                meta: vec![]
            };

            self.transaction_queue.push(transaction_event);

            self.handle_request_transaction_outcoming(2, vec![self.shared_memory_version.to_ne_bytes().to_vec()])?;

            return Ok(());
        }

        // Node disconnected
        else if transaction_kind == 1 {
            log::debug!("Transaction kind: `node_disconnected`");

            let bytes = event.data.get(2).unwrap();
            let node_id = utils::u128_from_ne_bytes(bytes).unwrap();

            if node_id != self.node_id {
                let mut to_del = None;
                for i in 0..self.nodes.len() {
                    let id = self.nodes.get(i).unwrap();

                    if node_id == *id {
                        to_del = Some(i);
                    }
                }

                if let Some(id) = to_del {
                    self.nodes.remove(id);
                }
            }

            log::debug!("Node id: {}", node_id);
        }

        else if transaction_kind == 2 {
            log::debug!("Transaction kind: `sync_shared_memory`");

            let bytes = event.data.get(2).unwrap();
            let version = utils::u128_from_ne_bytes(bytes).unwrap();
            self.shared_memory_version = version;

            self.shared_memory.clear();
            let fields_num = (event.data.len() - 3) / 2;
            for i in 0..fields_num {
                let index = (2 * i + 3) as usize;

                let key_bytes = event.data.get(index + 0).unwrap();
                let value_bytes = event.data.get(index + 1).unwrap();

                let key = utils::i32_from_ne_bytes(key_bytes).unwrap();
                let value = value_bytes.to_vec();

                self.shared_memory.insert(key, value);
            }

            log::info!("Shared memory synced");
        }

        // Update shared memory
        else if transaction_kind == 3 {
            log::debug!("Transaction kind: `update_shared_memory`");

            let version_bytes = event.data.get(2).unwrap();
            let key_bytes = event.data.get(3).unwrap();
            let value_bytes = event.data.get(4).unwrap();

            let version = utils::u128_from_ne_bytes(version_bytes).unwrap();
            let key = utils::i32_from_ne_bytes(key_bytes).unwrap();
            let value = value_bytes.to_vec();

            self.shared_memory_version = version;
            self.shared_memory.insert(key, value);

            if self.is_transaction_master {
                let event = proto_msg::Event {
                    dir: Some(proto_msg::event::Dir::Incoming as i32),
                    dest: Some(proto_msg::event::Dest::PluginMan as i32),
                    kind: proto_msg::event::Kind::TransactionSucceeded as i32,
                    data: vec![],
                    meta: event.meta
                };

                self.main_event_channel_tx.send(event).unwrap();
            }
        }

        else {
            log::debug!("Unknown transaction kind");
        }

        self.is_transaction = false;
        self.is_transaction_master = false;

        Ok(())
    }
}

impl From<FSMError> for NodeError {
    fn from(_: FSMError) -> Self {
        NodeError::InternalError
    }
}
