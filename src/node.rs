use std::{net::IpAddr, sync::mpsc::Sender};

use crate::event::{Event, EventChannel};

pub struct Node {
  pub ec: EventChannel,
  pub nodes: Vec<IpAddr>
}

impl Node {
  pub fn new(ec: EventChannel) -> Self {
    Self { ec, nodes: Vec::new() }
  }
}
