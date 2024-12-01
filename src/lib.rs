#![warn(clippy::all, clippy::pedantic, missing_docs)]
#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

mod dns;
mod request;
mod response;
mod uri;

pub use request::*;
pub use response::*;

use std::io;

/// GET the resource at an URL.
///
/// This is a convenience function over using [`Request::get`] and [`Request::send`].
///
/// # Errors
///
/// May error if the provided URL is invalid, or if network issues arise.
///
/// # Examples
///
/// ```rust
/// let response = request::get("localhost:8000").unwrap();
/// assert_eq!(response.status, 200);
/// ```
pub fn get(url: &str) -> Result<Response, io::Error> {
    Request::get(url).send()
}

/// POST a body to the URL.
///
/// This is a convenience function over using [`Request::post`] and [`Request::send`].
///
/// # Errors
///
/// May error if the provided URL is invalid, or if network issues arise.
///
/// # Examples
///
/// ```rust
/// let response = request::post("localhost:8000", "hello server!").unwrap();
/// assert_eq!(response.status, 501); // unsupported method
/// ```
pub fn post(url: &str, body: &str) -> Result<Response, io::Error> {
    Request::post(url, body).send()
}
