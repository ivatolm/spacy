mod server;

use server::Server;
use std::{sync::mpsc, net::TcpStream};
use common::utils;

fn main() {
    env_logger::init();

    let (main_event_channel_tx, _main_event_channel_rx) = mpsc::channel();

    let server = Server::new(main_event_channel_tx);
    let (_server_event_channel_tx, server_handle) = server.start();

    std::thread::sleep(std::time::Duration::from_secs(1));

    let ips = utils::get_ipv4_ips();
    let local_ip = ips.get(0).unwrap();
    let stream = TcpStream::connect((*local_ip, 32000));
    stream.unwrap();

    match server_handle.join() {
        Ok(_) => {},
        Err(_) => {}
    };
}
