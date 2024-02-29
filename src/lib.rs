#![warn(clippy::all, clippy::pedantic)]
#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

#[cfg(test)]
mod tests;

use once_cell::sync::Lazy;
use regex::Regex;
use std::{
    collections::HashMap,
    fmt,
    io::{BufRead, BufReader, Error as IoError, Write},
    iter,
    net::{IpAddr, Ipv4Addr, TcpStream, ToSocketAddrs, UdpSocket},
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
/// assert_eq!(response.status, 200)
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
        }
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
    pub fn send(&self) -> Result<Response, IoError> {
        // format the message
        let message = format!("{self}");
        dbg!(&message);

        // create the stream
        let host = resolve(host(self.url).unwrap())?;
        let mut stream = TcpStream::connect((host, 80))?;

        // send the message
        stream.write_all(message.as_bytes())?;

        // receive the response
        let lines = BufReader::new(stream)
            .lines()
            .map_while(Result::ok)
            .collect::<Vec<_>>();
        let received = lines.join("\n");

        // process response
        let response = Response::parse(&received).unwrap();

        Ok(response)
    }
}
impl<'a> fmt::Display for Request<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (method, path, host, body) = (
            self.method,
            path(self.url).ok_or(fmt::Error)?,
            host(self.url).ok_or(fmt::Error)?,
            self.body,
        );

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
    pub version: String,
    pub status: u16,
    pub reason: String,
    pub headers: HashMap<String, String>,
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

        // check if redirect
        if status == 301 {
            todo!()
        }

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
    Regex::new("(?:(?P<scheme>https?)://)?(?P<host>[0-9a-zA-Z:\\.\\-]+)(?P<path>/(?:.)*)?").unwrap()
});
#[allow(dead_code)]
fn scheme(url: &str) -> Option<&str> {
    URI_REGEX.captures(url)?.name("scheme").map(|m| m.as_str())
}
fn host(url: &str) -> Option<&str> {
    URI_REGEX.captures(url)?.name("host").map(|m| m.as_str())
}
fn path(url: &str) -> Option<&str> {
    URI_REGEX
        .captures(url)?
        .name("path")
        .map(|m| m.as_str())
        .or(Some("/"))
}

/// Resolve DNS request using system nameservers.
fn resolve(query: &str) -> Result<IpAddr, IoError> {
    // find name servers (platform-dependent)
    let servers = {
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
            ("8.8.8.8", 53).to_socket_addrs()?.collect::<Vec<_>>()
        }
    };

    // request dns resolution from nameservers
    let header: [u16; 6] = [0xabcd, 0x0100, 0x0001, 0x0000, 0x0000, 0x0000].map(|b: u16| b.to_be());
    let question: [u16; 2] = [0x0001, 0x0001].map(|b: u16| b.to_be());

    // convert query to standard dns name notation
    let ascii = query.chars().filter(char::is_ascii).collect::<String>();
    let name = ascii
        .split('.')
        .flat_map(|l| iter::once(u8::try_from(l.len()).unwrap_or(63)).chain(l.bytes().take(63)))
        .chain(iter::once(0))
        .collect::<Vec<u8>>();

    // construct the message
    let mut message = bytemuck::cast::<[u16; 6], [u8; 12]>(header).to_vec();
    message.extend(&name[..]);
    message.extend(bytemuck::cast_slice(&question));

    // create the socket
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect(&servers[..])?;

    // write dns lookup message
    socket.send_to(&message, &servers[..]).unwrap();

    // read dns response
    let mut buf = vec![0; 1024];
    let (n, _addr) = socket.recv_from(&mut buf)?;
    buf.resize(n, 0);

    // parse out the address
    let answers = &buf[message.len()..];
    let ip = &answers[12..];
    let address = IpAddr::V4(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]));

    Ok(address)
}
