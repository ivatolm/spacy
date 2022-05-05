use crate::event::EventChannel;

pub struct Plugin {
  pub ec: EventChannel,
  pub source: String
}

impl Plugin {
  pub fn new(ec: EventChannel, source: String) -> Self {
    Self { ec, source }
  }

  pub fn start(self) {

  }
}
