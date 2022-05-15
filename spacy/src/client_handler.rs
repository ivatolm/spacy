use std::{
  thread::{self, JoinHandle},
  net::TcpListener,
  io::{Read, Write}
};
use common::{
  tools,
  event::{EventKind, EventSender, Event, EventChannel},
  message::{self, proto_msg}
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

      let msg = message::deserialize_message(&buf[..size]).unwrap();
      let event_kind = EventKind::try_from(msg.cmd.unwrap()).unwrap();
      let event = Event::new(EventSender::ClientHandler, event_kind.clone(), msg.data);

      main_ec.tx.send(event).unwrap();

      if event_kind == EventKind::Other {
        let event = main_ec.rx.recv().unwrap();
        let msg = proto_msg::Message {
          cmd: Some(event_kind.to_int()),
          data: event.data
        };
        let msg = message::serialize_message(msg);
        stream.write(&msg).unwrap();
      }
    }
  }
}
