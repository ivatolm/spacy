use std::{
    collections::HashMap,
    thread
};
use common::fsm::{FSM, FSMError};

pub struct Server {
    fsm: FSM
}

pub enum ServerError {
    InternalError
}

impl Server {
    /*
    0 - initialization
    1 - waiting for events
    2 - handling events
    */

    pub fn new() -> Self {
        let fsm = FSM::new(0, HashMap::from([
            (0, vec![1]),
            (1, vec![2]),
            (2, vec![1])
        ]));

        Self { fsm }
    }

    pub fn start(mut self) -> thread::JoinHandle<Result<(), ServerError>> {
        thread::spawn(move || loop {
            match self.fsm.state {
                0 => self.init()?,
                1 => self.wait_event()?,
                2 => self.handle_event()?,
                _ => unreachable!()
            }
        })
    }

    fn init(&mut self) -> Result<(), ServerError> {
        self.fsm.transition(1)?;
        Ok(())
    }

    fn wait_event(&mut self) -> Result<(), ServerError> {
        self.fsm.transition(2)?;
        Ok(())
    }

    fn handle_event(&mut self) -> Result<(), ServerError> {
        let event = self.fsm.pop_event();
        match event.unwrap() {
            _ => {}
        }

        self.fsm.transition(1)?;
        Ok(())
    }

    pub fn push_event(&mut self, event: u8) {
        // Events will be transmitted via channels
        self.fsm.push_event(event);
    }
}

impl From<FSMError> for ServerError {
    fn from(_: FSMError) -> Self {
        ServerError::InternalError
    }
}
