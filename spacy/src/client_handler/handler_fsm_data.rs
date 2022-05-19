use std::collections::HashMap;

pub enum ClientHandlerStates {
  Err = 0,
  Init = 1,
  WaitEvent = 2
}

impl ClientHandlerStates {
  pub fn to_u8(&self) -> u8 {
    ClientHandlerStates::Init as u8
  }
}

impl TryFrom<u8> for ClientHandlerStates {
  type Error = ();

  fn try_from(value: u8) -> Result<Self, Self::Error> {
    match value {
      0 => Ok(ClientHandlerStates::Err),
      1 => Ok(ClientHandlerStates::Init),
      2 => Ok(ClientHandlerStates::WaitEvent),
      _ => Err(())
    }
  }
}

pub fn gen_transitions() -> HashMap::<u8, Vec<u8>> {
  let mut trainsitions = HashMap::<u8, Vec<u8>>::new();

  trainsitions.insert(ClientHandlerStates::Init.to_u8(), vec![
    ClientHandlerStates::WaitEvent.to_u8()
  ]);

  trainsitions
}
