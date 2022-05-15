mod client_handler;
mod node;
mod plugin;

use std::sync::mpsc;
use common::{event::{Event, EventSender, EventKind, EventChannel}};
use client_handler::ClientHandler;
use node::Node;
use plugin::Plugin;

fn main() {
  let (client_handler_tx, client_handler_rx) = mpsc::channel();
  let (node_tx, node_rx) = mpsc::channel();
  let (main_tx, main_rx) = mpsc::channel();

  let client_handler_ec = EventChannel::new(main_tx.clone(), client_handler_rx, client_handler_tx.clone());
  let client_handler = ClientHandler::new(32002, client_handler_ec);

  let node_ec = EventChannel::new(main_tx.clone(), node_rx, node_tx.clone());
  let node = Node::new(32000, 100, node_ec);

  let mut plugins_txs: Vec<mpsc::Sender<Event>> = Vec::new();
  let mut plugins = Vec::new();

  loop {
    let event = main_rx.recv().unwrap();

    match event.sender {
      EventSender::Node => match event.kind {
        EventKind::NewMessage | EventKind::Other => {
          let event = Event::new(EventSender::Main, event.kind, event.data);

          let tx = plugins_txs.get(0).unwrap();
          tx.send(event).unwrap();
        },
        _ => panic!()
      },
      EventSender::Plugin => match event.kind {
        EventKind::NoOp => {},
        EventKind::GetNodes => {
          let event = Event::new(EventSender::Main, event.kind, node.nodes());

          let tx = plugins_txs.get(0).unwrap();
          tx.send(event).unwrap();
        },
        EventKind::Broadcast => {
          node.broadcast(32000, event.data.get(0).unwrap().to_string());
        },
        EventKind::Other => {
          let event = Event::new(EventSender::Main, event.kind, event.data);
          client_handler_tx.send(event).unwrap();
        },
        _ => panic!()
      },
      EventSender::ClientHandler => match event.kind {
        EventKind::NewPlugin => {
          let (plugin_tx, plugin_rx) = mpsc::channel();
          let plugin_ec = EventChannel::new(main_tx.clone(), plugin_rx, plugin_tx.clone());
          let source = event.data.get(0).unwrap();

          let plugin = Plugin::new(32001, source.to_string(), plugin_ec);

          plugins_txs.push(plugin_tx);
          plugins.push(plugin);
        },
        EventKind::Other => {
          let event = Event::new(EventSender::Main, event.kind, event.data);

          let tx = plugins_txs.get(0).unwrap();
          tx.send(event).unwrap();
        },
        _ => panic!()
      },
      _ => break
    }
  }

  for plugin in plugins {
    plugin.stop();
  }
  node.stop();
  client_handler.stop();
}
