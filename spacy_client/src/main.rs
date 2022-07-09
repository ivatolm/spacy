use std::{
    net::TcpStream,
    io::Write,
    fs
};
use common::event::{
    self,
    proto_msg
};

fn main() {
    // Testing only

    let content = fs::read_to_string("plugins/basic.py").unwrap();
    let event = proto_msg::Event {
        dir: Some(proto_msg::event::Dir::Incoming as i32),
        dest: None,
        kind: proto_msg::event::Kind::NewPlugin as i32,
        data: vec![content.as_bytes().to_vec()]
    };

    let msg = event::serialize(event);

    let mut stream = TcpStream::connect(("192.168.0.106", 32000)).unwrap();
    stream.write(&msg).unwrap();
}
