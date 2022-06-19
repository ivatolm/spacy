use std::collections::{HashMap, HashSet};

use common::fsm;

fn main() -> Result<(), fsm::FSMError> {
    let mut fsm = fsm::FSM::new(0, HashMap::from([
        (0, HashSet::from([0]))
    ]));

    fsm.transition(0)?; // passes

    fsm.transition(1)?; // failes

    Ok(())
}
