use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, Shutdown};
use std::time::Duration;

use crate::event::{Event, EventChannel};
use crate::tools;

pub struct Node {
  pub ec: EventChannel,
  pub nodes: Vec<IpAddr>
}

impl Node {
  pub fn new(ec: EventChannel) -> Self {
    Self { ec, nodes: Vec::new() }
  }

  pub fn start(mut self) {
    self.scan(32000, 100);
  }

  fn scan(&mut self, port: u16, connection_timeout_millis: u64) {
    let mut nodes = Vec::new();

    loop {
      let interfaces = tools::interfaces();
      for iface in interfaces.iter() {
        let octet = tools::get_octet(iface.network());

        for i in 0..255 {
          let ip = IpAddr::V4(Ipv4Addr::new(octet[0], octet[1], octet[2], i));
          let socket_address = SocketAddr::new(ip, port);

          let stream = TcpStream::connect_timeout(
            &socket_address,
            Duration::from_millis(connection_timeout_millis));

          match stream {
            Ok(stream) => {
              nodes.push(ip);
              stream.shutdown(Shutdown::Both).unwrap()
            },
            Err(_) => {}
          }
        }
      }

      self.nodes = nodes.clone();
      nodes.clear();
    }
  }
}
