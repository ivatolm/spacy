mod tools;
mod event;
mod node;
mod plugin;

use std::sync::mpsc;
use event::EventChannel;
use node::Node;
use plugin::Plugin;

fn main() {
  let (node_tx, node_rx) = mpsc::channel();
  let (main_tx, main_rx) = mpsc::channel();

  let node_ec = EventChannel::new(main_tx.clone(), node_rx, node_tx);

  let node = Node::new(node_ec);
  let node_join_handlers = node.start();

  let mut plugins_join_handlers = Vec::new();

  loop {
    let event = main_rx.recv().unwrap();

    match event.sender() {
      "network" => match event.title() {
        "new_plugin" => {
          if event.data.len() != 1 {
            break;
          }

          let (plugin_tx, plugin_rx) = mpsc::channel();

          let plugin_ec = EventChannel::new(main_tx.clone(), plugin_rx, plugin_tx);
          let source = event.data.get(0).unwrap();

          let plugin = Plugin::new(plugin_ec, source.to_string());
          let join_handler = plugin.start();
          plugins_join_handlers.push(join_handler);
        },
        _ => break
      },
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
