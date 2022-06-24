use std::collections::{LinkedList, HashMap};

pub struct FSM {
    pub state: u8,
    queue: LinkedList<u8>,
    transition_table: HashMap<u8, Vec<u8>>
}

pub enum FSMError {
    TransitionError
}

impl FSM {
    pub fn new(init_state: u8, transition_table: HashMap<u8, Vec<u8>>) -> Self {
        Self {
            state: init_state,
            queue: LinkedList::new(),
            transition_table
        }
    }

    pub fn transition(&mut self, next_state: u8) -> Result<(), FSMError> {
        if let Some(states) = self.transition_table.get(&self.state) {
            if states.contains(&next_state) {
                self.state = next_state;
                return Ok(());
            }
        }

        Err(FSMError::TransitionError)
    }

    pub fn push_event(&mut self, event: u8) {
        self.queue.push_back(event);
    }

    pub fn pop_event(&mut self) -> Option<u8> {
        self.queue.pop_front()
    }
}
