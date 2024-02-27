use request::*;

fn main() {
    // create and send a simple request
    let response = Request::get("localhost:8000").send().unwrap();
    println!("response: {:#?}", response);
}
