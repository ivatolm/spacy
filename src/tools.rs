use std::{net::{IpAddr, Ipv4Addr}};

use pnet::{datalink, ipnetwork::IpNetwork};

use crate::event::EventKind;

pub fn interfaces() -> Vec<IpNetwork> {
  let interfaces = datalink::interfaces()
    .iter()
    .filter(|iface| iface.is_up() && !iface.is_loopback() && !iface.ips.is_empty())
    .map(|iface| iface.ips.get(0).unwrap().clone())
    .collect();

  interfaces
}

pub fn local_ip() -> IpAddr {
  let ifaces = interfaces();

  ifaces.get(0).unwrap().ip()
}

pub fn get_octet(address: IpAddr) -> [u8; 4] {
  address.to_string().parse::<Ipv4Addr>().unwrap().octets()
}

pub fn event_kind_from_string(x: &str) -> Result<EventKind, String> {
  match x {
    "new_plugin" => Ok(EventKind::NewPlugin),
    "new_message" => Ok(EventKind::NewMessage),
    "broadcast" => Ok(EventKind::Broadcast),
    _ => Err("Match not found".to_string())
  }
}
