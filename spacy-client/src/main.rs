use std::{net::TcpStream, io::Write};

use common::event::{proto_msg, self};

fn main() {
    // Testing only

    let event = proto_msg::Event {
        dir: proto_msg::event::Direction::Incoming as i32,
        kind: proto_msg::event::Kind::NewPlugin as i32,
        data: vec![vec![123]]
    };

    let msg = event::serialize(event);

    let mut stream = TcpStream::connect(("192.168.0.106", 32000)).unwrap();
    stream.write(&msg).unwrap();
}
