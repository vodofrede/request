fn main() {
    let response = request::Request::get("localhost:8000").send().unwrap();
    dbg!(&response);
}
