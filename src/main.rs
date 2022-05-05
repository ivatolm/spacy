mod tools;
mod event;
mod node;

use std::sync::mpsc;
use event::EventChannel;
use node::Node;

fn main() {
  let (tx, rx) = mpsc::channel();
  let ec = EventChannel::new(tx, rx);

  let node = Node::new(ec);
  node.start();
}
