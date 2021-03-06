use std::{
    net::{TcpStream, Shutdown},
    collections::HashMap,
    io::Write,
    os::unix::prelude::AsRawFd
};
use common::{
    fsm::FSM,
    event::{self, proto_msg}, utils
};
use nix::sys::{
    select::{select, FdSet},
    time::{TimeVal, TimeValLike}
};
use pyo3::{
    prelude::*
};


#[pyclass]
#[derive(Clone)]
struct SpacyKinds {
    pub kind_transaction_succeeded: i32,
    pub kind_transaction_failed: i32,
    pub kind_get_from_shared_memory: i32
}

#[pymethods]
impl SpacyKinds {
    #[new]
    pub fn new() -> Self {
        Self {
            kind_transaction_succeeded: proto_msg::event::Kind::TransactionSucceeded as i32,
            kind_transaction_failed: proto_msg::event::Kind::TransactionFailed as i32,
            kind_get_from_shared_memory: proto_msg::event::Kind::GetFromSharedMemory as i32
        }
    }

    #[getter]
    fn kind_transaction_succeeded(&mut self) -> i32 {
        self.kind_transaction_succeeded
    }

    #[getter]
    fn kind_transaction_failed(&mut self) -> i32 {
        self.kind_transaction_failed
    }

    #[getter]
    fn kind_get_from_shared_memory(&mut self) -> i32 {
        self.kind_get_from_shared_memory
    }
}

#[pyclass]
#[allow(dead_code)]
#[derive(Clone)]
struct SpacyEvent {
    pub kind: i32,
    pub data: Vec<Vec<u8>>,
    pub meta: Vec<Vec<u8>>
}

#[pymethods]
impl SpacyEvent {
    #[getter]
    fn kind(&self) -> i32 {
        self.kind
    }

    #[getter]
    fn data(&self) -> Vec<Vec<u8>> {
        self.data.to_owned()
    }

    #[getter]
    fn meta(&self) -> Vec<Vec<u8>> {
        self.meta.to_owned()
    }
}

#[pyclass(subclass)]
struct SpacyPlugin {
    fsm: FSM,
    stream: TcpStream,
    event_queue: Vec<SpacyEvent>
}

#[pymethods]
impl SpacyPlugin {
    // 0 - initialization
    // 1 - waiting for events
    // 2 - handle event
    // 3 - stop

    #[new]
    fn new() -> Self {
        let fsm = FSM::new(0, HashMap::from([
            (0, vec![1, 3]),
            (1, vec![2, 3]),
            (2, vec![1, 3]),
            (3, vec![])
        ]));

        // Connecting to the plugin manager
        let stream = TcpStream::connect(("127.0.0.1", 32002)).unwrap();

        Self {
            fsm,
            stream,
            event_queue: vec![]
        }
    }

    fn step(&mut self) {
        match self.fsm.state {
            0 => self.init(),
            1 => self.wait_event(),
            2 => self.handle_event(),
            3 => {
                self.stop();
                return;
            },
            _ => unreachable!()
        }
    }

    fn init(&mut self) {
        match self.fsm.transition(1) {
            Ok(_) => return,
            Err(_) => panic!(),
        };
    }

    fn wait_event(&mut self) {
        // Checking if there anything to read
        let mut readfds = FdSet::new();
        let fd = self.stream.as_raw_fd();
        readfds.insert(fd);

        let mut timeout = TimeVal::milliseconds(1);
        let result = select(None, &mut readfds, None, None, &mut timeout);
        if result.is_err() || !readfds.contains(fd) {
            return;
        }

        // Reading message until read
        let events = utils::read_events(&mut self.stream).unwrap();

        // If message length is zero, then plugin manager disconnected
        if events.len() != 0 {
            for event in events {
                self.fsm.push_event(event);
            }
        } else {
            self.stream.shutdown(Shutdown::Both).unwrap();

            match self.fsm.transition(3) {
                Ok(_) => return,
                Err(_) => panic!(),
            };
        }

        match self.fsm.transition(2) {
            Ok(_) => return,
            Err(_) => panic!(),
        };
    }

    fn handle_event(&mut self) {
        let event = self.fsm.pop_front_event().unwrap();

        let spacy_event = SpacyEvent {
            kind: event.kind,
            data: event.data,
            meta: event.meta
        };

        self.event_queue.push(spacy_event);

        match self.fsm.transition(1) {
            Ok(_) => return,
            Err(_) => panic!(),
        };
    }

    fn stop(&mut self) {
        panic!()
    }

    fn get_event(&mut self) -> Option<SpacyEvent> {
        match self.event_queue.len() {
            0 => None,
            _ => {
                Some(self.event_queue.remove(0))
            }
        }
    }

    fn shared_memory_push(&mut self, key: i32, value: Vec<u8>) {
        let event = proto_msg::Event {
            dir: Some(proto_msg::event::Dir::Outcoming as i32),
            dest: None,
            kind: proto_msg::event::Kind::UpdateSharedMemory as i32,
            data: vec![key.to_ne_bytes().to_vec(), value],
            meta: vec![]
        };
        self.stream.write(&event::serialize(event)).unwrap();
    }

    fn shared_memory_get(&mut self, key: i32) {
        let event = proto_msg::Event {
            dir: Some(proto_msg::event::Dir::Outcoming as i32),
            dest: None,
            kind: proto_msg::event::Kind::GetFromSharedMemory as i32,
            data: vec![key.to_ne_bytes().to_vec()],
            meta: vec![]
        };
        self.stream.write(&event::serialize(event)).unwrap();
    }

    fn respond_client(&mut self, data: Vec<Vec<u8>>, meta: Vec<Vec<u8>>) {
        let event = proto_msg::Event {
            dir: Some(proto_msg::event::Dir::Outcoming as i32),
            dest: None,
            kind: proto_msg::event::Kind::RespondClient as i32,
            data,
            meta
        };
        self.stream.write(&event::serialize(event)).unwrap();
    }

    fn execute(mut _self: PyRefMut<'_, Self>) {
        // Taking control of execution
        let gil = Python::acquire_gil();
        let py = gil.python();

        // Calling object method
        let obj = _self.into_py(py);
        obj.call_method1(py, "update", ()).unwrap();
    }

    // This method must be overwritten by the plugin
    fn update(&self, _event: SpacyEvent) -> SpacyEvent {
        unimplemented!();
    }
}

#[pymodule]
fn spacy_plugin(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<SpacyPlugin>()?;
    m.add_class::<SpacyKinds>()?;
    Ok(())
}
