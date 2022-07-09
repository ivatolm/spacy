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

pub fn get_octets(address: &std::net::IpAddr) -> Result<[u8; 4], std::net::AddrParseError> {
    let address_string = address.to_string();
    let address_ipv4 = address_string.parse::<std::net::Ipv4Addr>()?;
    let octets = address_ipv4.octets();
    Ok(octets)
}

pub fn u8_from_ne_bytes(bytes: &[u8]) -> Result<u8, std::array::TryFromSliceError> {
    Ok(u8::from_ne_bytes(bytes[0..bytes.len()].try_into()?))
}

pub fn u128_from_ne_bytes(bytes: &[u8]) -> Result<u128, std::array::TryFromSliceError> {
    Ok(u128::from_ne_bytes(bytes[0..bytes.len()].try_into()?))
}

pub fn i32_from_ne_bytes(bytes: &[u8]) -> Result<i32, std::array::TryFromSliceError> {
    Ok(i32::from_ne_bytes(bytes[0..bytes.len()].try_into()?))
}
