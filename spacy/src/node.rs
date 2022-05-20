use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, TcpListener};
use std::time::Duration;
use std::thread::{self, JoinHandle};
use std::sync::{Mutex, Arc};
use std::io::{Read, Write};

use common::events::MNEvents;
use common::{
  message::{self, proto_msg},
  event::{Event, EventChannel},
  tools
};

pub struct Node {
  pub nodes: Arc<Mutex<Vec<IpAddr>>>,
  pub scan_handle: JoinHandle<()>,
  pub listener_handle: JoinHandle<()>
}

impl Node {
  pub fn new(port: u16, ping_timeout_millis: u64, ec: EventChannel) -> Self {
    let nodes = Arc::new(Mutex::new(Vec::new()));

    let nodes_clone = nodes.clone();
    let scan_handle = thread::spawn(move || Self::scan(port, ping_timeout_millis, nodes_clone));
    let listener_handle = thread::spawn(move || Self::listener(port, ec));

    Self { nodes, scan_handle, listener_handle }
  }

  pub fn stop(self) {
    self.scan_handle.join().unwrap();
    self.listener_handle.join().unwrap();
    unimplemented!()
  }

  pub fn nodes(&self) -> Vec<String> {
    let nodes = self.nodes.lock().unwrap();
    let result = nodes.iter()
      .filter(|ip| ip.to_string() != tools::local_ip().to_string())
      .map(|ip| ip.to_string())
      .collect();

    result
  }

  pub fn broadcast(&self, port: u16, message: String) {
    let nodes = self.nodes.lock().unwrap();
    for node in nodes.iter() {
      if *node == tools::local_ip() {
        continue;
      }

      match TcpStream::connect((*node, port)) {
        Ok(mut stream) => { stream.write(message.as_bytes()).unwrap(); },
        Err(_) => {}
      }
    }
  }

  fn listener(port: u16, ec: EventChannel) {
    let listener = TcpListener::bind((tools::local_ip(), port)).unwrap();

    for stream in listener.incoming() {
      match stream {
        Ok(mut stream) => {
          let mut buf = [0u8; 16384];
          let size = stream.read(&mut buf).unwrap();

          if size == 0 { continue }

          let msg = message::deserialize_message(&buf[..size]).unwrap();
          let msg_data = msg.data.iter()
            .map(|x| x.as_bytes().to_vec())
            .collect();
          let event = Event::new(MNEvents::NewMessage as u8, msg_data);

          ec.tx.send(event).unwrap();

          // if event_kind == EventKind::Other {
          //   let event = ec.rx.recv().unwrap();
          //   let msg = proto_msg::Message {
          //     cmd: Some(event_kind.to_int()),
          //     data: event.data
          //   };
          //   stream.write(&message::serialize_message(msg)).unwrap();
          // }
        },
        Err(_) => {}
      }
    }
  }

  fn scan(port: u16, ping_timeout_millis: u64, nodes: Arc<Mutex<Vec<IpAddr>>>) {
    let mut upd_nodes = Vec::new();

    loop {
      let interfaces = tools::interfaces();
      for iface in interfaces.iter() {
        let octet = tools::get_octet(iface.network());

        for i in 0..255 {
          let ip = IpAddr::V4(Ipv4Addr::new(octet[0], octet[1], octet[2], i));
          let socket_address = SocketAddr::new(ip, port);

          let stream = TcpStream::connect_timeout(
            &socket_address,
            Duration::from_millis(ping_timeout_millis)
          );

          match stream {
            Ok(_) => {
              upd_nodes.push(ip);
            },
            Err(_) => {}
          }
        }
      }

      let mut nodes = nodes.lock().unwrap();
      *nodes = upd_nodes.clone();

      upd_nodes.clear();
    }
  }
}
