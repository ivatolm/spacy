use std::net::TcpListener;
use std::io::{Read, Write};
use std::process::Command;
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::lib::{
  protocol::Message,
  event::{Event, EventSender, EventKind, EventChannel},
  tools
};

pub struct Plugin {
  pub ec: EventChannel,
  pub source: String
}

impl Plugin {
  pub fn new(ec: EventChannel, source: String) -> Self {
    Self { ec, source }
  }

  pub fn start(self, port: u16) -> Vec<JoinHandle<()>> {
    let lbtx = self.ec.lbtx.clone();
    let (server_tx, server_rx) = mpsc::channel();
    let server_ec = EventChannel::new(lbtx.clone(), server_rx, server_tx.clone());

    let server_handle = thread::spawn(move || Self::server(port, server_ec));

    Command::new("python3")
      .arg("-c")
      .arg(&self.source)
      .spawn()
      .unwrap();

    let event_handler_handle = thread::spawn(move || self.event_handler(server_tx));

    vec![server_handle, event_handler_handle]
  }

  fn event_handler(self, server_tx: Sender<Event>) {
    loop {
      let event = self.ec.rx.recv().unwrap();

      match event.sender {
        EventSender::Plugin => {
          let event = Event::new(EventSender::Plugin, event.kind, event.data);
          self.ec.tx.send(event).unwrap();
        },
        EventSender::Main => {
          let event = Event::new(EventSender::Lb, event.kind, event.data);
          server_tx.send(event).unwrap();
        },
        _ => unreachable!()
      }
    }
  }

  fn server(port: u16, ec: EventChannel) {
    let listener = TcpListener::bind((tools::local_ip(), port)).unwrap();
    let (mut stream, _) = listener.accept().unwrap();

    loop {
      let mut buf = [0u8; 16384];
      let size = stream.read(&mut buf).unwrap();

      let data = String::from_utf8(buf[..size].to_vec()).unwrap();
      let message = Message::from(data).to_vec();

      let cmd = message.get(0).unwrap().to_string();
      let args = match message.len() - 1 {
        0 => vec![],
        _ => message.get(1..message.len()).unwrap().to_vec()
      };

      let event_kind: EventKind = cmd.try_into().unwrap();

      let event = Event::new(EventSender::Plugin, event_kind.clone(), args);
      ec.tx.send(event).unwrap();

      let event = match ec.rx.try_recv() {
        Ok(event) => {
          event
        },
        Err(_) => Event::new(EventSender::Lb, EventKind::NoOp, vec![])
      };

      match event.sender {
        EventSender::Lb => {
          let message = Message::from(event).to_string();
          stream.write(message.as_bytes()).unwrap();
        },
        _ => {}
      }

      thread::sleep(Duration::from_secs(1));
    }
  }
}
