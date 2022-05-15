use std::{
  thread::{self, JoinHandle},
  net::TcpListener,
  io::{Read, Write}
};
use common::{
  tools,
  protocol::Message, event::{EventKind, EventSender, Event, EventChannel}
};

pub struct ClientHandler {
  listener_handle: JoinHandle<()>
}

impl ClientHandler {
  pub fn new(port: u16, main_ec: EventChannel) -> Self {
    let listener_handle = thread::spawn(move || Self::listener(port, main_ec));
    Self { listener_handle }
  }

  pub fn stop(self) {
    self.listener_handle.join().unwrap();
    unimplemented!()
  }

  pub fn listener(port: u16, main_ec: EventChannel) {
    let listener = TcpListener::bind((tools::local_ip(), port)).unwrap();
    for stream in listener.incoming() {
      let mut stream = stream.unwrap();

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
      let event = match event_kind {
        EventKind::NewPlugin => {
          let message = Message::from(data);
          Event::new(EventSender::ClientHandler, event_kind.clone(), vec![message.skip_first()])
        },
        _ => {
          Event::new(EventSender::ClientHandler, event_kind.clone(), args)
        }
      };

      main_ec.tx.send(event).unwrap();

      if event_kind == EventKind::Other {
        let event = main_ec.rx.recv().unwrap();
        let message = Message::from(event).to_string();
        stream.write(message.as_bytes()).unwrap();
      }
    }
  }
}
