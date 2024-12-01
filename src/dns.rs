use std::{
    io, iter,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::LazyLock,
};

/// Resolve DNS request using system nameservers.
pub(crate) fn resolve(query: &str) -> Result<IpAddr, io::Error> {
    // todo: local overrides
    if query.starts_with("localhost") {
        return Ok(IpAddr::V4(Ipv4Addr::LOCALHOST));
    }

    // todo: dns caching
    // create dns query header: [id, flags, questions, answers, authority, additional]
    let header: [u16; 6] = [0xabcd, 0x0100, 0x0001, 0x0000, 0x0000, 0x0000].map(|b: u16| b.to_be());
    let question: [u16; 2] = [0x0001, 0x0001].map(|b: u16| b.to_be()); // [qtype, qclass] = [A, IN(ternet)]

    // convert query to standard dns name notation (max 63 characters for each label)
    let ascii = query.chars().filter(char::is_ascii).collect::<String>();
    let name = ascii
        .split('.')
        .flat_map(|l| {
            iter::once(u8::try_from(l.len()).unwrap_or(63).min(63)).chain(l.bytes().take(63))
        })
        .chain(iter::once(0))
        .collect::<Vec<u8>>();

    // construct the message
    let mut message = bytemuck::cast::<[u16; 6], [u8; 12]>(header).to_vec();
    message.extend(&name[..]);
    message.extend(bytemuck::cast_slice(&question));

    // create the socket
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect(&DNS_SERVERS[..])?;

    // write dns lookup message
    socket.send_to(&message, &DNS_SERVERS[..]).unwrap();

    // read dns response
    let mut buf = vec![0u8; 256];
    socket.peek_from(&mut buf)?;
    let n = socket.recv(&mut buf)?;
    buf.resize(n, 0);

    // parse out the address
    let ip = &buf.get(message.len()..).unwrap()[12..];
    let address = IpAddr::V4(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]));

    Ok(address)
}
static DNS_SERVERS: LazyLock<Vec<SocketAddr>> = LazyLock::new(|| {
    // find name servers (platform-dependent)
    #[cfg(unix)]
    {
        use std::{fs, net::ToSocketAddrs};
        let resolv = fs::read_to_string("/etc/resolv.conf").unwrap();
        let servers = resolv
            .lines()
            .filter_map(|l| l.split_once("nameserver ").map(|(_, s)| s.to_string()))
            .flat_map(|ns| (ns, 53).to_socket_addrs().unwrap())
            .collect::<Vec<_>>();

        servers
    }
    #[cfg(windows)]
    {
        // todo: get windows name servers
        vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 53)]
    }
    #[cfg(not(any(unix, windows)))]
    {
        vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 53)]
    }
});
