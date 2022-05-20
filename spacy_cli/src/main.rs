use std::{net::TcpStream, io::{stdin, Write, Read, self}, fs};
use common::{tools, message::{proto_msg, self}, events::MCEvents};

fn main() {
  loop {
    let mut input_string = String::new();
    print!("> ");
    io::stdout().flush().unwrap();
    stdin().read_line(&mut input_string).unwrap();

    let mut stream = TcpStream::connect((tools::local_ip(), 32002)).unwrap();

    if input_string == "upload\n" {
      let content = fs::read_to_string("plugin.py").unwrap();

      let msg = proto_msg::Message {
        cmd: Some(MCEvents::AddPlugin as i32),
        data: vec![content]
      };
      let msg = message::serialize_message(msg);
      stream.write(&msg).unwrap();
    } else {
      let input: Vec<String> = input_string.strip_suffix("\n").unwrap().to_string()
        .split(' ').map(|x| x.to_string()).collect();

      let msg = proto_msg::Message {
        cmd: Some(MCEvents::NewPluginCommand as i32),
        data: input
      };
      let msg = message::serialize_message(msg);
      stream.write(&msg).unwrap();
    }

    let mut buf = [0u8; 16384];
    let size = stream.read(&mut buf).unwrap();

    let msg = message::deserialize_message(&buf[..size]).unwrap();
    println!("{:?}, {:?}", msg.cmd, msg.data);
  }
}
