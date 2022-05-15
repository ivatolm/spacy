use prost::Message;

pub mod proto_msg {
  include!(concat!(env!("OUT_DIR"), "/common.message.rs"));
}

pub fn serialize_message(message: proto_msg::Message) -> Vec<u8> {
  let mut buf = Vec::new();
  buf.reserve(message.encoded_len());
  message.encode(&mut buf).unwrap();
  buf
}

pub fn deserialize_message(buf: &[u8]) -> Result<proto_msg::Message, prost::DecodeError> {
  proto_msg::Message::decode(buf)
}
