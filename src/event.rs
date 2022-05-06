use std::sync::mpsc::{Sender, Receiver};

pub struct Event {
  pub sender: EventSender,
  pub kind: EventKind,
  pub data: Vec<String>
}

pub enum EventSender {
  Lb,
  Main,
  Node,
  Plugin
}

pub enum EventKind {
  NewPlugin,
  NewMessage,
  GetNodes,
  Broadcast
}

pub struct EventChannel {
  pub tx: Sender<Event>,
  pub rx: Receiver<Event>,
  pub lbtx: Sender<Event>
}

impl Event {
  pub fn new(sender: EventSender, kind: EventKind, data: Vec<String>) -> Self {
    Self { sender, kind, data }
  }
}

impl EventChannel {
  pub fn new(tx: Sender<Event>, rx: Receiver<Event>, lbtx: Sender<Event>) -> Self {
    Self { tx, rx, lbtx }
  }
}
