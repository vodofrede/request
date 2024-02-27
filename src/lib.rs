#![warn(clippy::all, clippy::pedantic)]
#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

#[cfg(test)]
mod tests;

use once_cell::sync::Lazy;
use regex::Regex;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Error as IoError, Write},
    net,
};

/// An HTTP request.
#[derive(Debug, Clone)]
pub struct Request<'a> {
    /// Request URL.
    uri: &'a str,
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
    pub fn new(uri: &'a str, method: Method) -> Self {
        Self {
            uri,
            method,
            headers: HashMap::new(),
            body: "",
        }
    }

    /// Construct a new GET request.
    pub fn get(uri: &'a str) -> Self {
        Request::new(uri, Method::GET)
    }

    /// Construct a new POST request.
    pub fn post(uri: &'a str) -> Self {
        Request::new(uri, Method::POST)
    }

    /// Dispatch the request.
    pub fn send(&self) -> Result<Response, IoError> {
        // format the message: Method Request-URI HTTP-Version CRLF headers CRLF message-body
        // todo: properly format the headers
        let message = format!(
            "{:?} {} HTTP/1.1\r\n{:?}\r\n{}\r\n",
            self.method, self.uri, self.headers, self.body
        );

        // create the stream
        let mut stream = net::TcpStream::connect(self.uri)?;

        // send the message
        stream.write(message.as_bytes())?;

        // receive the response
        let lines = BufReader::new(stream)
            .lines()
            .map(|l| l.unwrap())
            .take_while(|l| !l.is_empty())
            .collect::<Vec<_>>();
        let received = lines.join("\n");

        // process response
        let response = Response::parse(&received).unwrap();

        Ok(response)
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
    pub status: u64,
    pub reason: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}
impl Response {
    pub fn parse(message: &str) -> Result<Self, &'static str> {
        // construct a regex: HTTP-Version Status-Code Reason-Phrase CRLF headers CRLF message-body
        static REGEX: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"(?P<version>HTTP/\d\.\d) (?P<status>\d+) (?P<reason>[a-zA-Z ]+)(?:\n(?P<headers>(?:.+\n)+))?(?:\n(?P<body>(?:.+\n?)+))?").unwrap()
        });

        // parse the response
        let Some(parts) = REGEX.captures(message) else {
            Err("invalid message")?
        };
        let version = parts["version"].to_string();
        let status = parts["status"].parse().unwrap();
        let reason = parts["reason"].to_string();

        // parse headers
        let headers = parts
            .name("headers")
            .map(|m| m.as_str())
            .unwrap_or("")
            .lines()
            .map(|l| l.split_once(": ").unwrap())
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

    pub fn is_ok(&self) -> bool {
        self.status == 200
    }
}
