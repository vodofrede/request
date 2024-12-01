use regex::Regex;
use std::{collections::HashMap, sync::LazyLock};

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
    pub body: String,
}
impl Response {
    /// Parse the raw HTTP response into a structured [`Request`].
    pub(crate) fn parse(message: &str) -> Result<Self, &'static str> {
        // construct a regex: HTTP-Version Status-Code Reason-Phrase CRLF headers CRLF message-body
        static MSG_REGEX: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(r"(?P<version>HTTP\/\d\.\d) (?P<status>\d+) (?P<reason>[a-zA-Z ]+)(?:\r?\n(?P<headers>(?:.+\r?\n)+))?(?:\r?\n(?P<body>[\S\s]*))?").unwrap()
        });

        // parse the response
        let Some(parts) = MSG_REGEX.captures(message) else {
            Err("invalid message")?
        };
        let version = parts["version"].to_string();
        let status = parts["status"].parse().unwrap();
        let reason = parts["reason"].to_string();

        // parse headers
        let headers = parts.name("headers").map_or("", |m| m.as_str());
        let headers = headers
            .lines()
            .filter_map(|l| l.split_once(": "))
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect::<HashMap<String, String>>();

        // parse body
        let body = parts
            .name("body")
            .map_or(String::new(), |m| m.as_str().to_string());

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

#[cfg(test)]
mod tests {
    use crate::Response;

    #[test]
    fn https_redirect() {
        let message = r"HTTP/1.1 301 Moved Permanently
Location: https://archlinux.org/

";

        let response = Response::parse(message).unwrap();
        dbg!(&response);
        assert_eq!(response.version, "HTTP/1.1".to_string());
        assert_eq!(response.status, 301);
        assert_eq!(response.reason, "Moved Permanently");
        assert_eq!(
            response.headers,
            std::collections::HashMap::from([(
                "Location".to_string(),
                "https://archlinux.org/".to_string()
            )])
        );
        assert_eq!(response.body, String::new());
    }
}
