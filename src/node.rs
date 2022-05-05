use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, Shutdown, TcpListener};
use std::time::Duration;
use std::thread::{self, JoinHandle};
use std::sync::{Mutex, Arc, mpsc::Sender};
use std::io::Read;

use crate::event::{Event, EventChannel};
use crate::tools;

pub struct Node {
  pub ec: EventChannel,
  pub nodes: Arc<Mutex<Vec<IpAddr>>>
}

impl Node {
  pub fn new(ec: EventChannel) -> Self {
    Self { ec, nodes: Arc::new(Mutex::new(Vec::new())) }
  }

  pub fn start(self) -> Vec<JoinHandle<()>> {
    let nodes = self.nodes.clone();
    let scan_handle = thread::spawn(move || Self::scan(32000, 100, nodes));

    let tx = self.ec.tx.clone();
    let listener_handle = thread::spawn(move || Self::listener(32000, tx));

    vec![scan_handle, listener_handle]
  }

  fn listener(port: u16, tx: Sender<Event>) {
    let listener = TcpListener::bind((tools::local_ip(), port)).unwrap();

    for stream in listener.incoming() {
      match stream {
        Ok(mut stream) => {
          let mut buf = [0u8; 1024];
          let size = stream.read(&mut buf).unwrap();

          if size == 0 {
            continue
          }

          let data = String::from_utf8(buf[..size].to_vec()).unwrap();
          let (cmd, args) = data.split_once(' ').unwrap();

          let event = Event::new(
            "network".to_string(),
            cmd.to_string(),
            vec![args.to_string()]);

          tx.send(event).unwrap();
        },
        Err(_) => {}
      }
    }
  }

  fn scan(port: u16, connection_timeout_millis: u64, nodes: Arc<Mutex<Vec<IpAddr>>>) {
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
            Duration::from_millis(connection_timeout_millis));

          match stream {
            Ok(stream) => {
              upd_nodes.push(ip);
              stream.shutdown(Shutdown::Both).unwrap()
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
