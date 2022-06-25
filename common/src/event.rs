use prost::Message;

pub mod proto_msg {
  include!(concat!(env!("OUT_DIR"), "/common.event.rs"));
}

pub fn serialize(event: proto_msg::Event) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.reserve(event.encoded_len());
    event.encode(&mut buf).unwrap();
    buf
}

pub fn deserialize(buf: &[u8]) -> Result<proto_msg::Event, prost::DecodeError> {
    proto_msg::Event::decode(buf)
}
