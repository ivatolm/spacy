mod tools;
mod event;
mod node;

use std::sync::mpsc;
use event::EventChannel;
use node::Node;

fn main() {
  let (tx1, rx1) = mpsc::channel();
  let (tx2, rx2) = mpsc::channel();

  let ec = EventChannel::new(tx2, rx1, tx1);

  let node = Node::new(ec);
  let node_join_handlers = node.start();

  for handler in node_join_handlers {
    handler.join().unwrap();
  }
}
