use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, TcpListener};
use std::sync::mpsc;
use std::time::Duration;
use std::thread::{self, JoinHandle};
use std::sync::{Mutex, Arc, mpsc::Sender};
use std::io::{Read, Write};

use crate::event::{Event, EventSender, EventKind, EventChannel};
use crate::protocol::Message;
use crate::tools;

pub struct Node {
  pub ec: EventChannel,
  pub nodes: Arc<Mutex<Vec<IpAddr>>>
}

impl Node {
  pub fn new(ec: EventChannel) -> Self {
    Self { ec, nodes: Arc::new(Mutex::new(Vec::new())) }
  }

  pub fn start(self, port: u16, ping_timeout_millis: u64) -> Vec<JoinHandle<()>> {
    let nodes = self.nodes.clone();
    let scan_handle = thread::spawn(move || Self::scan(port, ping_timeout_millis, nodes));

    let lbtx = self.ec.lbtx.clone();
    let (listener_tx, listener_rx) = mpsc::channel();

    let listener_ec = EventChannel::new(lbtx, listener_rx, listener_tx.clone());
    let listener_handle = thread::spawn(move || Self::listener(port, listener_ec));

    let event_handler_handle = thread::spawn(move || self.event_handler(port, listener_tx));

    vec![scan_handle, listener_handle, event_handler_handle]
  }

  fn event_handler(self, port: u16, listener_tx: Sender<Event>) {
    loop {
      let event = self.ec.rx.recv().unwrap();

      match event.sender {
        EventSender::Lb => {
          let event = Event::new(EventSender::Node, event.kind, event.data);
          self.ec.tx.send(event).unwrap();
        },
        EventSender::Main => match event.kind {
          EventKind::GetNodes => {
            let nodes = self.nodes.lock().unwrap();
            let result = nodes.iter()
              .map(|ip| ip.to_string())
              .collect();

            let event = Event::new(EventSender::Node, event.kind, result);
            self.ec.tx.send(event).unwrap();
          },
          EventKind::Broadcast => {
            let event = Event::new(event.sender, EventKind::NewMessage, event.data);
            let message = Message::from(event).to_string();

            let nodes = self.nodes.lock().unwrap();
            for node in nodes.iter() {
              match TcpStream::connect((*node, port)) {
                Ok(mut stream) => { stream.write(message.as_bytes()).unwrap(); },
                Err(_) => {}
              }
            }
          },
          EventKind::Other => {
            let event = Event::new(EventSender::Node, event.kind, event.data);
            listener_tx.send(event).unwrap();
          },
          _ => unreachable!()
        },
        _ => unreachable!()
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

          if size == 0 {
            continue;
          }

          let data = String::from_utf8(buf[..size].to_vec()).unwrap();
          let message = Message::from(data.clone()).to_vec();

          let cmd = message.get(0).unwrap().to_string();
          let args = match message.len() - 1 {
            0 => vec![],
            _ => message.get(1..message.len()).unwrap().to_vec()
          };

          let event_kind: EventKind = cmd.try_into().unwrap();
          let event = match event_kind  {
            EventKind::NewPlugin => {
              let message = Message::from(data);
              Event::new(EventSender::Lb, event_kind.clone(), vec![message.skip_first()])
            },
            _ => {
              Event::new(EventSender::Lb, event_kind.clone(), args)
            }
          };

          ec.tx.send(event).unwrap();

          if event_kind == EventKind::Other {
            let event = ec.rx.recv().unwrap();
            let message = Message::from(event).to_string();
            stream.write(message.as_bytes()).unwrap();
          }
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
