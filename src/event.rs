use std::sync::mpsc::{Sender, Receiver};

pub struct Event {
  pub sender: String,
  pub title: String,
  pub data: Vec<String>
}

pub struct EventChannel {
  pub tx: Sender<Event>,
  pub rx: Receiver<Event>
}

impl Event {
  pub fn new(sender: String, title: String, data: Vec<String>) -> Self {
    Self { sender, title, data }
  }
}

impl EventChannel {
  pub fn new(tx: Sender<Event>, rx: Receiver<Event>) -> Self {
    Self { tx, rx }
  }
}
