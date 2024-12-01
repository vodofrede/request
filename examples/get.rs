fn main() {
    let response = request::get("http://httpforever.com/").unwrap();
    dbg!(response);

    let response = request::get("http://archlinux.org/").unwrap();
    dbg!(response);
}
