use std::collections::HashMap;

enum StateMachineError {
  UnknownState,
  TrainsitionError
}

struct StateMachine {
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

  pub fn transition(&self, target_state: u8) -> Result<(), StateMachineError> {
    match self.map.get(&self.state) {
      Some(states) => {
        match states.contains(&target_state) {
          true => Ok(()),
          false => Err(StateMachineError::TrainsitionError)
        }
      }
      None => Err(StateMachineError::UnknownState)
    }
  }
}
