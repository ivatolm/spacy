use std::net::TcpListener;
use std::io::{Read, Write};
use std::process::Command;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use common::{
  protocol::Message,
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
        Ok(event) => event,
        Err(_) => Event::new(EventSender::Lb, EventKind::NoOp, vec![])
      };

      let message = Message::from(event).to_string();
      stream.write(message.as_bytes()).unwrap();

      thread::sleep(Duration::from_secs(1));
    }
  }
}
