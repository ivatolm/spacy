use std::collections::HashMap;

use log::warn;

#[derive(Debug)]
pub enum StateMachineError {
  UnknownState,
  TrainsitionError
}

pub struct StateMachine {
  pub state: u8,
  pub map: HashMap<u8, Vec<u8>>
}

impl StateMachine {
  pub fn new(init_state: u8, transition_map: HashMap<u8, Vec<u8>>) -> Self {
    Self {
      state: init_state,
      map: transition_map
    }
  }

  pub fn transition(&mut self, target_state: u8) -> Result<(), StateMachineError> {
    if self.state == target_state {
      warn!("Transitioning into the same state");
    }

    match self.map.get(&self.state) {
      Some(states) => {
        match states.contains(&target_state) {
          true => {
            self.state = target_state;
            Ok(())
          },
          false => Err(StateMachineError::TrainsitionError)
        }
      }
      None => Err(StateMachineError::UnknownState)
    }
  }
}
