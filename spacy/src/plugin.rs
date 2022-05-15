use std::net::TcpListener;
use std::io::{Read, Write};
use std::process::Command;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use common::{
  message::{self, proto_msg},
  event::{Event, EventSender, EventKind, EventChannel},
  tools
};

pub struct Plugin {
  pub server_handle: JoinHandle<()>
}

impl Plugin {
  pub fn new(port: u16, source:String, ec: EventChannel) -> Self {
    let server_handle = thread::spawn(move || Self::server(port, ec));

    Command::new("python3")
      .arg("-c")
      .arg(source)
      .spawn()
      .unwrap();

    Self { server_handle }
  }

  pub fn stop(self) {
    self.server_handle.join().unwrap();
    unimplemented!()
  }

  fn server(port: u16, ec: EventChannel) {
    let listener = TcpListener::bind((tools::local_ip(), port)).unwrap();
    let (mut stream, _) = listener.accept().unwrap();

    loop {
      let mut buf = [0u8; 16384];
      let size = stream.read(&mut buf).unwrap();

      let msg = message::deserialize_message(&buf[..size]).unwrap();
      let event_kind = EventKind::try_from(msg.cmd.unwrap()).unwrap();
      let event = Event::new(EventSender::Plugin, event_kind.clone(), msg.data);

      ec.tx.send(event).unwrap();

      let event = match ec.rx.try_recv() {
        Ok(event) => event,
        Err(_) => Event::new(EventSender::Lb, EventKind::NoOp, vec![])
      };

      let msg = proto_msg::Message {
        cmd: Some(event.kind.to_int()),
        data: event.data
      };
      let msg = message::serialize_message(msg);
      stream.write(&msg).unwrap();

      thread::sleep(Duration::from_secs(1));
    }
  }
}
