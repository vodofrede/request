use request::Request;

fn main() {
    let response = Request::get("http://httpforever.com/").send().unwrap();
    dbg!(&response);

    let response = Request::get("http://archlinux.org/").send().unwrap();
    dbg!(&response);
}
