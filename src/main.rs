mod tools;
mod event;
mod node;
mod plugin;

use std::sync::mpsc;
use event::{Event, EventSender, EventKind, EventChannel};
use node::Node;
use plugin::Plugin;

fn main() {
  let (node_tx, node_rx) = mpsc::channel();
  let (main_tx, main_rx) = mpsc::channel();

  let node_ec = EventChannel::new(main_tx.clone(), node_rx, node_tx.clone());

  let node = Node::new(node_ec);
  let node_join_handlers = node.start(32000, 100);

  let mut plugins_txs = Vec::new();
  let mut plugins_join_handlers = Vec::new();

  loop {
    let event = main_rx.recv().unwrap();

    match event.sender {
      EventSender::Node => match event.kind {
        EventKind::NewPlugin => {
          let (plugin_tx, plugin_rx) = mpsc::channel();
          let plugin_ec = EventChannel::new(main_tx.clone(), plugin_rx, plugin_tx.clone());
          let source = event.data.get(0).unwrap();

          let plugin = Plugin::new(plugin_ec, source.to_string());
          let join_handler = plugin.start();

          plugins_txs.push(plugin_tx);
          plugins_join_handlers.push(join_handler);
        },
        EventKind::NewMessage => {},
        EventKind::GetNodes => {
          let event = Event::new(EventSender::Main, event.kind, event.data);

          let tx = plugins_txs.get(0).unwrap();
          tx.send(event).unwrap();
        },
        _ => panic!()
      },
      EventSender::Plugin => match event.kind {
        EventKind::GetNodes => {
          let event = Event::new(EventSender::Main, event.kind, event.data);
          node_tx.send(event).unwrap();
        },
        EventKind::Broadcast => {
          let event = Event::new(EventSender::Main, EventKind::Broadcast, event.data);
          node_tx.send(event).unwrap();

          let event = Event::new(EventSender::Main, EventKind::Broadcast, Vec::new());
          let tx = plugins_txs.get(0).unwrap();
          tx.send(event).unwrap();
        },
        _ => panic!()
      }
      _ => break
    }
  }

  for handler in node_join_handlers {
    handler.join().unwrap();
  }

  for plugin_handlers in plugins_join_handlers {
    for handler in plugin_handlers {
      handler.join().unwrap();
    }
  }
}
