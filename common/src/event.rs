use std::sync::mpsc::{Sender, Receiver};

pub struct Event {
  pub kind: u8,
  pub data: Vec<Vec<u8>>,
  pub meta: Vec<u8>
}

pub struct EventChannel {
  pub tx: Sender<Event>,
  pub rx: Receiver<Event>,
  pub lbtx: Sender<Event>
}

impl Event {
  pub fn new(kind: u8, data: Vec<Vec<u8>>, meta: Vec<u8>) -> Self {
    Self { kind, data, meta }
  }
}

impl EventChannel {
  pub fn new(tx: Sender<Event>, rx: Receiver<Event>, lbtx: Sender<Event>) -> Self {
    Self { tx, rx, lbtx }
  }
}
