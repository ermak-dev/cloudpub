//! PROXY protocol v2 implementation
//!
//! Implements HAProxy's PROXY protocol version 2 for passing client connection
//! information to backend servers in a binary header format.
//!
//! Reference: https://www.haproxy.org/download/2.9/doc/proxy-protocol.txt

use std::net::{IpAddr, SocketAddr};

/// PROXY protocol v2 signature (12 bytes)
const PROXY_V2_SIGNATURE: [u8; 12] = [
    0x0D, 0x0A, 0x0D, 0x0A, 0x00, 0x0D, 0x0A, 0x51, 0x55, 0x49, 0x54, 0x0A,
];

/// Version and command byte values
const PROXY_V2_VERSION: u8 = 0x20; // Version 2
const PROXY_CMD_PROXY: u8 = 0x01; // PROXY command

/// Address family values
const AF_UNSPEC: u8 = 0x00;
const AF_INET: u8 = 0x10; // IPv4
const AF_INET6: u8 = 0x20; // IPv6

/// Transport protocol values
const STREAM: u8 = 0x01; // TCP

/// Builds a PROXY protocol v2 header for TCP connections
///
/// # Arguments
/// * `client_addr` - The original client's socket address
/// * `server_addr` - The server's socket address (destination)
///
/// # Returns
/// A byte vector containing the complete PROXY v2 header
pub fn build_proxy_v2_header(client_addr: &SocketAddr, server_addr: &SocketAddr) -> Vec<u8> {
    let mut header = Vec::with_capacity(52); // Max size for IPv6

    // 1. Signature (12 bytes)
    header.extend_from_slice(&PROXY_V2_SIGNATURE);

    // Determine address family and protocol
    let (af_proto, addr_len, addr_data) = match (client_addr.ip(), server_addr.ip()) {
        (IpAddr::V4(src_ip), IpAddr::V4(dst_ip)) => {
            // IPv4 + TCP
            let af_proto = AF_INET | STREAM;
            let mut data = Vec::with_capacity(12);
            data.extend_from_slice(&src_ip.octets()); // 4 bytes src addr
            data.extend_from_slice(&dst_ip.octets()); // 4 bytes dst addr
            data.extend_from_slice(&client_addr.port().to_be_bytes()); // 2 bytes src port
            data.extend_from_slice(&server_addr.port().to_be_bytes()); // 2 bytes dst port
            (af_proto, 12u16, data)
        }
        (IpAddr::V6(src_ip), IpAddr::V6(dst_ip)) => {
            // IPv6 + TCP
            let af_proto = AF_INET6 | STREAM;
            let mut data = Vec::with_capacity(36);
            data.extend_from_slice(&src_ip.octets()); // 16 bytes src addr
            data.extend_from_slice(&dst_ip.octets()); // 16 bytes dst addr
            data.extend_from_slice(&client_addr.port().to_be_bytes()); // 2 bytes src port
            data.extend_from_slice(&server_addr.port().to_be_bytes()); // 2 bytes dst port
            (af_proto, 36u16, data)
        }
        (IpAddr::V4(src_ip), IpAddr::V6(_)) => {
            // Mixed: convert IPv4 to IPv6-mapped address
            let src_v6 = src_ip.to_ipv6_mapped();
            let dst_v6 = match server_addr.ip() {
                IpAddr::V6(ip) => ip,
                _ => unreachable!(),
            };
            let af_proto = AF_INET6 | STREAM;
            let mut data = Vec::with_capacity(36);
            data.extend_from_slice(&src_v6.octets());
            data.extend_from_slice(&dst_v6.octets());
            data.extend_from_slice(&client_addr.port().to_be_bytes());
            data.extend_from_slice(&server_addr.port().to_be_bytes());
            (af_proto, 36u16, data)
        }
        (IpAddr::V6(_), IpAddr::V4(dst_ip)) => {
            // Mixed: convert IPv4 to IPv6-mapped address
            let src_v6 = match client_addr.ip() {
                IpAddr::V6(ip) => ip,
                _ => unreachable!(),
            };
            let dst_v6 = dst_ip.to_ipv6_mapped();
            let af_proto = AF_INET6 | STREAM;
            let mut data = Vec::with_capacity(36);
            data.extend_from_slice(&src_v6.octets());
            data.extend_from_slice(&dst_v6.octets());
            data.extend_from_slice(&client_addr.port().to_be_bytes());
            data.extend_from_slice(&server_addr.port().to_be_bytes());
            (af_proto, 36u16, data)
        }
    };

    // 2. Version and command (1 byte)
    header.push(PROXY_V2_VERSION | PROXY_CMD_PROXY);

    // 3. Address family and protocol (1 byte)
    header.push(af_proto);

    // 4. Address length (2 bytes, big-endian)
    header.extend_from_slice(&addr_len.to_be_bytes());

    // 5. Address data (variable length)
    header.extend_from_slice(&addr_data);

    header
}

/// Builds a minimal PROXY protocol v2 header for LOCAL command (no address info)
///
/// This is used when the connection is health-check or internal and doesn't
/// need client address information passed to the backend.
pub fn build_proxy_v2_local_header() -> Vec<u8> {
    let mut header = Vec::with_capacity(16);

    // Signature
    header.extend_from_slice(&PROXY_V2_SIGNATURE);

    // Version (2) + Command (LOCAL = 0)
    header.push(PROXY_V2_VERSION); // 0x20 = version 2, command LOCAL

    // Address family UNSPEC + protocol UNSPEC
    header.push(AF_UNSPEC);

    // Address length = 0
    header.extend_from_slice(&0u16.to_be_bytes());

    header
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

    #[test]
    fn test_proxy_v2_header_ipv4() {
        let client = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 100), 54321));
        let server = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 1), 25565));

        let header = build_proxy_v2_header(&client, &server);

        // Check signature
        assert_eq!(&header[0..12], &PROXY_V2_SIGNATURE);

        // Check version + command
        assert_eq!(header[12], 0x21); // v2 + PROXY

        // Check address family + protocol
        assert_eq!(header[13], 0x11); // AF_INET + STREAM

        // Check address length
        assert_eq!(&header[14..16], &12u16.to_be_bytes());

        // Total header size for IPv4: 16 + 12 = 28 bytes
        assert_eq!(header.len(), 28);

        // Verify source IP
        assert_eq!(&header[16..20], &[192, 168, 1, 100]);

        // Verify destination IP
        assert_eq!(&header[20..24], &[10, 0, 0, 1]);

        // Verify source port (54321 = 0xD431)
        assert_eq!(&header[24..26], &54321u16.to_be_bytes());

        // Verify destination port (25565 = 0x63DD)
        assert_eq!(&header[26..28], &25565u16.to_be_bytes());
    }

    #[test]
    fn test_proxy_v2_header_ipv6() {
        let client = SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1),
            54321,
            0,
            0,
        ));
        let server = SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 2),
            25565,
            0,
            0,
        ));

        let header = build_proxy_v2_header(&client, &server);

        // Check signature
        assert_eq!(&header[0..12], &PROXY_V2_SIGNATURE);

        // Check version + command
        assert_eq!(header[12], 0x21); // v2 + PROXY

        // Check address family + protocol
        assert_eq!(header[13], 0x21); // AF_INET6 + STREAM

        // Check address length
        assert_eq!(&header[14..16], &36u16.to_be_bytes());

        // Total header size for IPv6: 16 + 36 = 52 bytes
        assert_eq!(header.len(), 52);
    }

    #[test]
    fn test_proxy_v2_local_header() {
        let header = build_proxy_v2_local_header();

        // Check signature
        assert_eq!(&header[0..12], &PROXY_V2_SIGNATURE);

        // Check version + command (LOCAL = 0)
        assert_eq!(header[12], 0x20); // v2 + LOCAL

        // Check address family + protocol
        assert_eq!(header[13], 0x00); // UNSPEC

        // Check address length = 0
        assert_eq!(&header[14..16], &0u16.to_be_bytes());

        // Total size: 16 bytes
        assert_eq!(header.len(), 16);
    }
}
