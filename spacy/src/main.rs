mod node;
mod server;
mod plugin_man;

use node::Node;
use plugin_man::PluginMan;
use server::Server;
use std::{
    sync::mpsc,
    thread,
    time
};
use common::event::{self, proto_msg};

fn main() {
    env_logger::init();

    let (main_event_channel_tx, main_event_channel_rx) = mpsc::channel();

    let main_event_channel_tx_clone = main_event_channel_tx.clone();
    let mut node = Node::new(main_event_channel_tx_clone);
    let node_event_channel_tx = node.start();

    let main_event_channel_tx_clone = main_event_channel_tx.clone();
    let server = Server::new(main_event_channel_tx_clone);
    let (server_event_channel_tx, _server_handle) = server.start();

    let main_event_channel_tx_clone = main_event_channel_tx.clone();
    let mut plugin_man = PluginMan::new(main_event_channel_tx_clone);
    let plugin_man_event_channel_tx = plugin_man.start();

    loop {
        let event_res = main_event_channel_rx.try_recv();
        if let Ok(event) = event_res {
            if event.kind == proto_msg::event::Kind::NewPluginManEvent as i32 {
                log::info!("Received new event for plugin manager");

                // Sending an event to plugin manager
                let first_arg = event.data.get(0).unwrap();
                let actual_event = event::deserialize(first_arg).unwrap();
                plugin_man_event_channel_tx.send(actual_event).unwrap();
            }

            else if event.kind == proto_msg::event::Kind::NewNodeEvent as i32 {
                log::debug!("Recevied new event for node");

                // Sending an event to node
                let first_arg = event.data.get(0).unwrap();
                let actual_event = event::deserialize(first_arg).unwrap();
                node_event_channel_tx.send(actual_event).unwrap();
            }

            else if event.kind == proto_msg::event::Kind::NewServerEvent as i32 {
                log::debug!("Recevied new event for server");

                // Sending an event to server
                let first_arg = event.data.get(0).unwrap();
                let actual_event = event::deserialize(first_arg).unwrap();
                server_event_channel_tx.send(actual_event).unwrap();
            }
        }

        node.step().unwrap();
        plugin_man.step().unwrap();

        thread::sleep(time::Duration::from_secs(5));
    }

    // match server_handle.join() {
    //     Ok(_) => {},
    //     Err(_) => {}
    // };
}
