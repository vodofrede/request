fn main() {
    let response = request::Request::get("http://httpforever.com/")
        .send()
        .unwrap();
    dbg!(&response);
}
