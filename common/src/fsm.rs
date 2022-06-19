use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub enum FSMError {
    FSMTransitionError
}

pub struct FSM {
    state: u8,
    transition_table: HashMap<u8, HashSet<u8>>
}

impl FSM {
    pub fn new(init_state: u8, transition_table: HashMap<u8, HashSet<u8>>) -> Self {
        Self { state: init_state, transition_table }
    }

    pub fn transition(&mut self, new_state: u8) -> Result<(), FSMError> {
        if let Some(row) = self.transition_table.get(&self.state) {
            if row.get(&new_state).is_some() {
                self.state = new_state;
                return Ok(());
            }
        }

        Err(FSMError::FSMTransitionError)
    }

    pub fn state(&self) -> u8 {
        self.state
    }
}
