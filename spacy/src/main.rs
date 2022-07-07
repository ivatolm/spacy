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

fn main() {
    env_logger::init();

    let (main_event_channel_tx, _main_event_channel_rx) = mpsc::channel();

    let main_event_channel_tx_clone = main_event_channel_tx.clone();
    let mut node = Node::new(main_event_channel_tx_clone);
    let _node_event_channel_tx = node.start();

    let main_event_channel_tx_clone = main_event_channel_tx.clone();
    let server = Server::new(main_event_channel_tx_clone);
    let (_server_event_channel_tx, _server_handle) = server.start();

    let main_event_channel_tx_clone = main_event_channel_tx.clone();
    let mut plugin_man = PluginMan::new(main_event_channel_tx_clone);
    let _plugin_man_event_channel_tx = plugin_man.start();

    loop {
        match node.step() {
            Ok(_) => {}
            Err(_) => {
                log::error!("Error occured while updating `node`");
                panic!();
            }
        }

        plugin_man.step().unwrap();

        thread::sleep(time::Duration::from_millis(100));
    }

    // match server_handle.join() {
    //     Ok(_) => {},
    //     Err(_) => {}
    // };
}
