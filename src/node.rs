use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, Shutdown, TcpListener};
use std::time::Duration;
use std::thread::{self, JoinHandle};
use std::sync::{Mutex, Arc, mpsc::Sender};
use std::io::{Read, Write};

use crate::event::{Event, EventSender, EventKind, EventChannel};
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

    let lbtx = self.ec.lbtx.clone();
    let listener_handle = thread::spawn(move || Self::listener(32000, lbtx));

    let event_handler_handle = thread::spawn(move || self.event_handler(32000));

    vec![scan_handle, listener_handle, event_handler_handle]
  }

  fn event_handler(self, port: u16) {
    loop {
      let event = self.ec.rx.recv().unwrap();

      match event.sender {
        EventSender::Lb => {
          println!("EventHandler received event from 'network'");

          let event = Event::new(
            EventSender::Node,
            event.kind,
            event.data
          );

          self.ec.tx.send(event).unwrap();
        },
        EventSender::Main => {
          println!("EventHandler received event from 'main'");

          match event.kind {
            EventKind::Broadcast => {
              println!("Broadcasting a message...");

              let nodes = self.nodes.lock().unwrap();
              for node in nodes.iter() {
                match TcpStream::connect((*node, port)) {
                  Ok(mut stream) => {
                    let data = "new_message ".to_string() + event.data.get(0).unwrap();
                    let bytes = data.as_bytes();
                    stream.write(bytes).unwrap();
                    stream.shutdown(Shutdown::Both).unwrap();
                  },
                  Err(_) => {}
                }
              }
            },
            _ => unreachable!()
          }
        },
        _ => unreachable!()
      }
    }
  }

  fn listener(port: u16, tx: Sender<Event>) {
    let listener = TcpListener::bind((tools::local_ip(), port)).unwrap();

    for stream in listener.incoming() {
      match stream {
        Ok(mut stream) => {
          let mut buf = [0u8; 1024];
          let size = stream.read(&mut buf).unwrap();

          if size == 0 {
            continue;
          }

          let data = String::from_utf8(buf[..size].to_vec()).unwrap();
          let (cmd, args) = data.split_once(' ').unwrap();

          let event_kind = match cmd {
            "new_plugin" => EventKind::NewPlugin,
            "new_message" => EventKind::NewMessage,
            _ => panic!()
          };

          let event = Event::new(
            EventSender::Lb,
            event_kind,
            vec![args.to_string()]
          );

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
              stream.shutdown(Shutdown::Both).unwrap();
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
