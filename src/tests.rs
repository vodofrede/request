use super::*;

#[test]
fn get() {
    // create and send a simple request
    let response = Request::get("http://httpforever.com/").send().unwrap();
    assert_eq!(response.status, 200);
}

#[test]
fn post() {}
