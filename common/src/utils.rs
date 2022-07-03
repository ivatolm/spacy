fn get_interfaces() -> Vec<pnet::ipnetwork::IpNetwork> {
    let interfaces: Vec<pnet::ipnetwork::IpNetwork> = pnet::datalink::interfaces()
        .into_iter()
        .filter(|iface| iface.is_up() && !iface.is_loopback() && !iface.ips.is_empty())
        .flat_map(|iface| iface.ips.into_iter())
        .collect();

    interfaces
}

pub fn get_ipv4_ips() -> Vec<std::net::IpAddr> {
    get_interfaces()
        .into_iter()
        .filter(|iface| iface.is_ipv4())
        .map(|iface| iface.ip())
        .collect()
}

pub fn get_networks_and_masks() -> Vec<(std::net::IpAddr, std::net::IpAddr)> {
    get_interfaces()
        .into_iter()
        .map(|iface| (iface.network(), iface.mask()))
        .collect()
}

pub fn i32_from_ne_bytes(bytes: &[u8]) -> Result<i32, std::array::TryFromSliceError> {
    Ok(i32::from_ne_bytes(bytes[0..bytes.len()].try_into()?))
}
