use std::sync::mpsc::{Sender, Receiver};

pub struct Event {
  pub sender: String,
  pub title: String,
  pub data: Vec<String>
}

pub struct EventChannel {
  pub tx: Sender<Event>,
  pub rx: Receiver<Event>,
  pub lbtx: Sender<Event>
}

impl Event {
  pub fn new(sender: String, title: String, data: Vec<String>) -> Self {
    Self { sender, title, data }
  }

  pub fn sender(&self) -> &str {
    &self.sender
  }

  pub fn title(&self) -> &str {
    &self.title
  }
}

impl EventChannel {
  pub fn new(tx: Sender<Event>, rx: Receiver<Event>, lbtx: Sender<Event>) -> Self {
    Self { tx, rx, lbtx }
  }
}
