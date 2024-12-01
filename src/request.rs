use crate::{dns, uri, Response};
use std::{collections::HashMap, fmt, io, io::prelude::*, iter, net::TcpStream};

/// An HTTP request builder.
///
/// # Examples
///
/// A simple GET request:
/// ```rust
/// use request::Request;
///
/// // ... start a local server on port 8000 ...
/// let response = request::get("localhost:8000").unwrap();
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
///     request.to_string(),
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
    /// assert_eq!(request.to_string(), "GET / HTTP/1.1\r\nHost: example.org\r\n\r\n");
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
    /// # Errors
    ///
    /// May error if the response is invalid or if too many redirects are issued.
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
        let name = uri::host(self.url).ok_or(io::Error::new(
            io::ErrorKind::InvalidInput,
            "url host part is invalid",
        ))?;
        let host = dns::resolve(name)?;
        let port = uri::port(self.url).map_or(80, |p| p.parse::<u16>().unwrap_or(80));
        let mut stream = TcpStream::connect((host, port))?;

        // send the message
        stream.write_all(message.as_bytes())?;

        // receive the response
        // todo: allow larger responses by resizing response buffer
        let mut buffer = vec![0u8; 4096];
        let length = stream.read(&mut buffer)?;
        buffer.resize(length, 0);
        let received = String::from_utf8(buffer)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "received invalid data"))?;

        // process response
        let response = Response::parse(&received)
            .map_err(|s| io::Error::new(io::ErrorKind::InvalidData, s))?;

        // check for redirects
        match response.status {
            300..400 => {
                // redirect
                if self.redirects == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "maximum redirect limit reached",
                    ));
                }
                let location = response.headers.get("Location").ok_or(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "no location header provided in redirect",
                ))?;
                let request = self.clone().redirects(self.redirects - 1).url(location);
                (response.status == 303)
                    .then(|| request.send())
                    .unwrap_or_else(|| request.method(Method::GET).send())
            }
            _ => Ok(response),
        }
    }
}
impl<'a> fmt::Display for Request<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let method = self.method;
        let path = uri::path(self.url).ok_or(fmt::Error)?;
        let host = uri::host(self.url).ok_or(fmt::Error)?;
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
