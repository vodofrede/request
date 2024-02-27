use super::*;

#[test]
fn get() {
    common::server();

    // create and send a simple request
    let response = Request::get("localhost:8000").send().unwrap();
    println!("response: {:#?}", response);
}

#[test]
fn post() {}

mod common {
    use std::{
        io::{BufRead, BufReader, Write},
        net, thread,
    };

    pub fn server() {
        let listener = net::TcpListener::bind("localhost:8000").expect("port is in use.");
        thread::spawn(move || {
            listener
                .incoming()
                .filter_map(Result::ok)
                .for_each(|mut t| {
                    let _ = BufReader::new(&mut t)
                        .lines()
                        .take_while(|l| !l.as_ref().unwrap().is_empty())
                        .collect::<Vec<_>>();
                    t.write(b"HTTP/1.1 200 OK\r\n\r\n").unwrap();
                })
        });
    }
}
