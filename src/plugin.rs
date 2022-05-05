use std::net::TcpListener;
use std::io::Read;
use std::process::Command;
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};

use crate::event::{EventChannel, Event};
use crate::tools;

pub struct Plugin {
  pub ec: EventChannel,
  pub source: String
}

impl Plugin {
  pub fn new(ec: EventChannel, source: String) -> Self {
    Self { ec, source }
  }

  pub fn start(self) -> Vec<JoinHandle<()>> {
    let lbtx = self.ec.lbtx.clone();
    let server_handle = thread::spawn(move || Self::server(32001, lbtx));

    Command::new("python3")
      .arg("-c")
      .arg(&self.source)
      .spawn()
      .unwrap();

    let event_handler_handle = thread::spawn(move || self.event_handler());

    vec![server_handle, event_handler_handle]
  }

  fn event_handler(self) {
    loop {
      let event = self.ec.rx.recv().unwrap();

      match event.sender() {
        "plugin" => println!("EventHandler received event from 'plugin'"),
        "main" => println!("EventHandler received event from 'main'"),
        _ => unreachable!()
      }
    }
  }

  fn server(port: u16, tx: Sender<Event>) {
    let listener = TcpListener::bind((tools::local_ip(), port)).unwrap();
    let (mut stream, _) = listener.accept().unwrap();

    let mut buf = [0u8; 1024];
    let size = stream.read(&mut buf).unwrap();

    let data = String::from_utf8(buf[..size].to_vec()).unwrap();
    let (cmd, args) = data.split_once(' ').unwrap();

    let event = Event::new(
      "plugin".to_string(),
      cmd.to_string(),
      vec![args.to_string()]);

    tx.send(event).unwrap();
  }
}
