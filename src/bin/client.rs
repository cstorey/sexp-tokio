extern crate futures;
extern crate tokio_core as tokio;
extern crate tokio_service as service;
extern crate tokio_proto as proto;
extern crate miniature_goggles as goggles;
extern crate env_logger;
use std::env::args;
use std::io;

use proto::pipeline::{self, Frame};
use tokio::reactor::{Core, Handle};
use service::Service;
use futures::{Poll, Async};
use futures::stream;

enum Empty {}

pub fn main() {
    env_logger::init().unwrap();

    let mut core = Core::new().unwrap();

    let addr = "127.0.0.1:12345".parse().unwrap();

    let client = goggles::client::connect(core.handle(), &addr);

    // - one that returns a future that we can 'await' on.
    let resp = client.call("Hello".to_string());
    let res: Result<String, goggles::Error> = core.run(resp);
    println!("RESPONSE: {:?}", res);
}
