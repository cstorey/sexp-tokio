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

    let service = goggles::server::serve(&core.handle(),
                                         addr,
                                         service::simple_service(|msg| {
                                             println!("GOT: {:?}", msg);
                                             Ok(msg)
                                         }));


    // Now our client. We use the same reactor as for the server - usually though this would be
    // done in a separate program most likely on a separate machine.
    let client = goggles::client::connect(core.handle(), &addr);

    // The connect call returns us a ClientHandle that allows us to use the 'Service' as a function
    // - one that returns a future that we can 'await' on.
    let resp = client.call("Hello".to_string());
    let res: Result<String, io::Error> = core.run(resp);
    println!("RESPONSE: {:?}", res);
}
