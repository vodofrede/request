use regex::Regex;
use std::sync::LazyLock;

static URI_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new("(?:(?P<scheme>https?)://)?(?P<host>[0-9a-zA-Z\\.\\-]+)(?:\\:(?P<port>\\d+))?(?P<path>/(?:.)*)?").unwrap()
});
#[allow(dead_code)]
pub(crate) fn scheme(url: &str) -> Option<&str> {
    URI_REGEX.captures(url)?.name("scheme").map(|m| m.as_str())
}
pub(crate) fn host(url: &str) -> Option<&str> {
    URI_REGEX.captures(url)?.name("host").map(|m| m.as_str())
}
pub(crate) fn port(url: &str) -> Option<&str> {
    URI_REGEX.captures(url)?.name("port").map(|m| m.as_str())
}
pub(crate) fn path(url: &str) -> Option<&str> {
    URI_REGEX
        .captures(url)?
        .name("path")
        .map(|m| m.as_str())
        .or(Some("/"))
}
