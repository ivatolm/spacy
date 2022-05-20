use std::collections::HashMap;

pub enum ClientHandlerState {
  Err = 0,
  Init = 1,
  WaitEvent = 2
}

impl TryFrom<u8> for ClientHandlerState {
  type Error = ();

  fn try_from(value: u8) -> Result<Self, Self::Error> {
    match value {
      0 => Ok(ClientHandlerState::Err),
      1 => Ok(ClientHandlerState::Init),
      2 => Ok(ClientHandlerState::WaitEvent),
      _ => Err(())
    }
  }
}

pub fn gen_transitions() -> HashMap::<u8, Vec<u8>> {
  let mut trainsitions = HashMap::<u8, Vec<u8>>::new();

  trainsitions.insert(ClientHandlerState::Init as u8, vec![
    ClientHandlerState::WaitEvent as u8
  ]);

  trainsitions
}
