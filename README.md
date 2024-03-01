# Request

A library for making HTTP requests.

# Examples

A simple GET request:
```rust
use request::Request;

// ... start a local server on port 8000 ...
let request = Request::get("localhost:8000");
let response = request.send().unwrap();
assert_eq!(response.status, 200);
```

Adding headers:
```rust
use request::Request;

// ... start a local server on port 8000 ...
let response = Request::get("localhost:8000")
    .header("Authorization", "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==")
    .send()
    .unwrap();
assert_eq!(response.status, 200);
```

A POST request with serialized JSON data.
```rust
use request::Request;

#[derive(miniserde::Serialize)]
struct Example { code: u32, message: String }

let data = Example { code: 123, message: "hello".to_string() };
let json = miniserde::json::to_string(&data);
let request = Request::post("example.org/api", &json);
assert_eq!(
    format!("{request}"),
    "POST /api HTTP/1.1\r\nHost: example.org\r\n\r\n{\"code\":123,\"message\":\"hello\"}"
);
```