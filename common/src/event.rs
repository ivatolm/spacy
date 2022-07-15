use prost::Message;

pub mod proto_msg {
  include!(concat!(env!("OUT_DIR"), "/common.event.rs"));
}

pub fn serialize(event: proto_msg::Event) -> Vec<u8> {
    event.encode_length_delimited_to_vec()
}

pub fn deserialize(mut buf: &[u8]) -> (Vec<proto_msg::Event>, &[u8]) {
	let mut events = vec![];

	// In system without acknowledgment events may pile up,
	// so we trying to take as much as we can

	loop {
		let event = match proto_msg::Event::decode_length_delimited(buf) {
			Ok(event) => event,
			Err(_) => {
				return (events, buf);
			}
		};
		events.push(event.clone());

		let serialized_event = serialize(event);
		let read_bytes = serialized_event.len();

		buf = &buf[read_bytes..];
		if buf.len() == 0 {
			break;
		}
	}

	(events, buf)
}
