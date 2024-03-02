#![warn(clippy::all, clippy::pedantic, missing_docs)]
#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

use once_cell::sync::Lazy;
use regex::Regex;
use std::{
    collections::HashMap,
    fmt,
    io::{self, Read, Write},
    iter,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, UdpSocket},
};

/// An HTTP request.
///
/// # Examples
///
/// A simple GET request:
/// ```rust
/// use request::Request;
///
/// // ... start a local server on port 8000 ...
/// let request = Request::get("localhost:8000");
/// let response = request.send().unwrap();
/// assert_eq!(response.status, 200);
/// ```
///
/// Adding headers:
/// ```rust
/// use request::Request;
///
/// // ... start a local server on port 8000 ...
/// let response = Request::get("localhost:8000")
///     .header("Authorization", "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==")
///     .send()
///     .unwrap();
/// assert_eq!(response.status, 200);
/// ```
///
/// A POST request with serialized JSON data.
/// ```rust
/// use request::Request;
///
/// #[derive(miniserde::Serialize)]
/// struct Example { code: u32, message: String }
///
/// let data = Example { code: 123, message: "hello".to_string() };
/// let json = miniserde::json::to_string(&data);
/// let request = Request::post("example.org/api", &json);
/// assert_eq!(
///     format!("{request}"),
///     "POST /api HTTP/1.1\r\nHost: example.org\r\n\r\n{\"code\":123,\"message\":\"hello\"}"
/// );
/// ```
#[must_use]
#[derive(Debug, Clone)]
pub struct Request<'a> {
    /// Request URL.
    url: &'a str,
    /// An HTTP method. GET by default.
    method: Method,
    /// Request headers.
    headers: HashMap<&'a str, &'a str>,
    /// Request body.
    body: &'a str,
    /// How many redirects are followed before an error is emitted.
    redirects: usize,
}

impl<'a> Request<'a> {
    /// Create a new request.
    ///
    /// Convenience functions are provided for each HTTP method [`Request::get`], [`Request::post`] etc.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use request::*;
    /// let request = Request::new("example.org", Method::GET);
    /// assert_eq!(format!("{request}"), "GET / HTTP/1.1\r\nHost: example.org\r\n\r\n");
    /// ```
    pub fn new(url: &'a str, method: Method) -> Self {
        Self {
            url,
            method,
            headers: HashMap::new(),
            body: "",
            redirects: 4,
        }
    }

    /// Set the HTTP method of the request.
    pub fn method(self, method: Method) -> Self {
        let mut request = self;
        request.method = method;
        request
    }

    /// Set the URL of the request.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use request::*;
    /// let request = Request::get("http://example.org/a").url("http://example.org/b");
    /// assert_eq!(format!("{request}"), "GET /b HTTP/1.1\r\nHost: example.org\r\n\r\n");
    /// ```
    pub fn url(self, url: &'a str) -> Self {
        let mut request = self;
        request.url = url;
        request
    }

    /// Add a body to the request.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use request::*;
    /// let request = Request::new("example.org", Method::POST).body("Hello Server!");
    /// assert_eq!(format!("{request}"), "POST / HTTP/1.1\r\nHost: example.org\r\n\r\nHello Server!");
    /// ```
    pub fn body(self, body: &'a str) -> Self {
        let mut request = self;
        request.body = body;
        request
    }

    /// Add a header to the request.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use request::*;
    /// let request = Request::get("localhost").header("Accept", "*/*");
    /// ```
    pub fn header(self, key: &'a str, value: &'a str) -> Self {
        let mut request = self;
        request.headers.insert(key, value);
        request
    }

    /// Set the maximum allowed redirects.
    pub fn redirects(self, max: usize) -> Self {
        let mut request = self;
        request.redirects = max;
        request
    }

    /// Construct a new GET request.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use request::*;
    /// let request = Request::get("example.org");
    /// assert_eq!(format!("{request}"), "GET / HTTP/1.1\r\nHost: example.org\r\n\r\n");
    /// ```
    pub fn get(url: &'a str) -> Self {
        Request::new(url, Method::GET)
    }

    /// Construct a new POST request.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use request::*;
    /// let request = Request::post("example.org", r#"{ "hello": "world"}"#);
    /// assert_eq!(format!("{request}"), "POST / HTTP/1.1\r\nHost: example.org\r\n\r\n{ \"hello\": \"world\"}")
    /// ```
    pub fn post(url: &'a str, body: &'a str) -> Self {
        Request::new(url, Method::POST).body(body)
    }

    /// Dispatch the request.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use request::*;
    /// // ... start a local server on port 8000 ...
    /// let request = Request::get("localhost:8000");
    /// let response = request.send().expect("request failed");
    /// assert_eq!(response.status, 200);
    /// ```
    pub fn send(&self) -> Result<Response, io::Error> {
        // format the message
        let message = format!("{self}");

        // create the stream
        let host = resolve(host(self.url).unwrap())?;
        let port = port(self.url).unwrap_or("80").parse::<u16>().unwrap();
        let mut stream = TcpStream::connect((host, port))?;

        // send the message
        stream.write_all(message.as_bytes())?;

        // receive the response
        // todo: allow larger responses by resizing response buffer
        let mut buf = vec![0u8; 4096];
        let n = stream.read(&mut buf)?;
        buf.resize(n, 0);
        let received = String::from_utf8(buf).unwrap();

        // check for redirects
        let status: u16 = received[9..12].parse().unwrap();
        if (300..400).contains(&status) {
            // todo: error for maximum redirect limit reached
            assert!(self.redirects > 0, "maximum redirect limit reached");
            let location = received
                .lines()
                .find_map(|l| l.strip_prefix("Location: "))
                .unwrap(); // todo: error for missing location in redirect
            let request = self.clone().redirects(self.redirects - 1).url(location);
            return (status == 303)
                .then(|| request.send())
                .unwrap_or_else(|| request.method(Method::GET).send());
        }

        // process response
        let response = Response::parse(&received).unwrap();

        Ok(response)
    }
}
impl<'a> fmt::Display for Request<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let method = self.method;
        let path = path(self.url).ok_or(fmt::Error)?;
        let host = host(self.url).ok_or(fmt::Error)?;
        let body = self.body;
        let headers = iter::once(format!("Host: {host}"))
            .chain(self.headers.iter().map(|(k, v)| format!("{k}: {v}")))
            .collect::<Vec<_>>()
            .join("\r\n");

        // format: Method Request-URI HTTP-Version CRLF headers CRLF CRLF message-body
        write!(f, "{method:?} {path} HTTP/1.1\r\n{headers}\r\n\r\n{body}")
    }
}

/// HTTP methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(missing_docs)]
pub enum Method {
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    CONNECT,
    OPTIONS,
    TRACE,
    PATCH,
}

/// An HTTP response.
#[derive(Debug, Clone)]
pub struct Response {
    /// HTTP version.
    ///
    /// Should be one of HTTP/1.0, HTTP/1.1, HTTP/2.0, or HTTP/3.0.
    pub version: String,
    /// Status code.
    ///
    /// 100-199: info, 200-299: success, 300-399: redir, 400-499: client error, 500-599: server error.
    pub status: u16,
    /// Message associated to the status code.
    pub reason: String,
    /// Map of headers.
    pub headers: HashMap<String, String>,
    /// Message body.
    pub body: Option<String>,
}
impl Response {
    /// Parse the raw HTTP response into a structured [`Request`].
    fn parse(message: &str) -> Result<Self, &'static str> {
        // construct a regex: HTTP-Version Status-Code Reason-Phrase CRLF headers CRLF message-body
        static MSG_REGEX: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"(?P<version>HTTP/\d\.\d) (?P<status>\d+) (?P<reason>[a-zA-Z ]+)(?:\n(?P<headers>(?:.+\n)+))?(?:\n(?P<body>[\S\s]*))?").unwrap()
        });

        // parse the response
        let Some(parts) = MSG_REGEX.captures(message) else {
            Err("invalid message")?
        };
        let version = parts["version"].to_string();
        let status = parts["status"].parse().unwrap();
        let reason = parts["reason"].to_string();

        // parse headers
        let headers = parts
            .name("headers")
            .map_or("", |m| m.as_str())
            .lines()
            .filter_map(|l| l.split_once(": "))
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect::<HashMap<String, String>>();

        // parse body
        let body = parts.name("body").map(|m| m.as_str().to_string());

        // construct the response
        let response = Response {
            version,
            status,
            reason,
            headers,
            body,
        };

        Ok(response)
    }
}

static URI_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new("(?:(?P<scheme>https?)://)?(?P<host>[0-9a-zA-Z\\.\\-]+)(?:\\:(?P<port>\\d+))?(?P<path>/(?:.)*)?").unwrap()
});
#[allow(dead_code)]
fn scheme(url: &str) -> Option<&str> {
    URI_REGEX.captures(url)?.name("scheme").map(|m| m.as_str())
}
fn host(url: &str) -> Option<&str> {
    URI_REGEX.captures(url)?.name("host").map(|m| m.as_str())
}
fn port(url: &str) -> Option<&str> {
    URI_REGEX.captures(url)?.name("port").map(|m| m.as_str())
}
fn path(url: &str) -> Option<&str> {
    URI_REGEX
        .captures(url)?
        .name("path")
        .map(|m| m.as_str())
        .or(Some("/"))
}

/// Resolve DNS request using system nameservers.
fn resolve(query: &str) -> Result<IpAddr, io::Error> {
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
static DNS_SERVERS: Lazy<Vec<SocketAddr>> = Lazy::new(|| {
    // find name servers (platform-dependent)
    #[cfg(unix)]
    {
        use std::fs;
        let resolv = fs::read_to_string("/etc/resolv.conf")?;
        let servers = resolv
            .lines()
            .filter_map(|l| l.split_once("nameserver ").map(|(_, s)| s.to_string()))
            .flat_map(|ns| ns.to_socket_addrs().into_iter().flatten())
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

#[cfg(test)]
mod tests;
