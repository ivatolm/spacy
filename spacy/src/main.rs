mod node;
mod server;

use node::Node;
use server::Server;
use std::sync::mpsc;

fn main() {
    env_logger::init();

    let (main_event_channel_tx, _main_event_channel_rx) = mpsc::channel();

    let main_event_channel_tx_clone = main_event_channel_tx.clone();
    let mut node = Node::new(main_event_channel_tx_clone);
    let _node_event_channel_tx = node.start();

    let main_event_channel_tx_clone = main_event_channel_tx.clone();
    let server = Server::new(main_event_channel_tx_clone);
    let (_server_event_channel_tx, server_handle) = server.start();

    match server_handle.join() {
        Ok(_) => {},
        Err(_) => {}
    };
}
