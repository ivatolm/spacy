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

#[derive(Clone, PartialEq)]
pub enum EventKind {
  NewPlugin,
  NewMessage,
  GetNodes,
  Broadcast,
  Other
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

impl EventKind {
  pub fn to_string(&self) -> String {
    match self {
      Self::NewPlugin => "new_plugin".to_string(),
      Self::NewMessage => "new_message".to_string(),
      Self::GetNodes => "get_nodes".to_string(),
      Self::Broadcast => "broadcast".to_string(),
      Self::Other => "other".to_string()
    }
  }
}

impl TryFrom<&str> for EventKind {
  type Error = ();

  fn try_from(value: &str) -> Result<Self, Self::Error> {
    match value {
      "new_plugin" => Ok(EventKind::NewPlugin),
      "new_message" => Ok(EventKind::NewMessage),
      "get_nodes" => Ok(EventKind::GetNodes),
      "broadcast" => Ok(EventKind::Broadcast),
      "other" => Ok(EventKind::Other),
      _ => Err(())
    }
  }
}

impl TryFrom<String> for EventKind {
  type Error = ();

  fn try_from(value: String) -> Result<Self, Self::Error> {
    EventKind::try_from(value.as_str())
  }
}
