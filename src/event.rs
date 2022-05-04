use std::sync::mpsc::{Sender, Receiver};

pub struct Event {
  pub sender: String,
  pub title: String,
  pub data: Vec<String>
}

pub struct EventChannel {
  pub tx: Sender<String>,
  pub rx: Receiver<String>
}

impl Event {
  pub fn new(sender: String, title: String, data: Vec<String>) -> Self {
    Self { sender, title, data }
  }
}

impl EventChannel {
  pub fn new(tx: Sender<String>, rx: Receiver<String>) -> Self {
    Self { tx, rx }
  }
}
