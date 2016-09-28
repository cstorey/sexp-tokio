
extern crate futures;
extern crate tokio_core as tokio;
extern crate tokio_line as line;
extern crate tokio_service as service;
extern crate tokio_proto as proto;
extern crate env_logger;
use std::env::args;
use std::io;

use proto::pipeline::{self, Frame};
use tokio::reactor::{Core, Handle};
use service::Service;
use futures::{Poll, Async};
use futures::stream;

enum Empty {}

struct MyClient {
    handle: Handle,
    client: proto::Client<String, String, stream::Empty<Empty, io::Error>, io::Error>,
}

struct MyTransport;

impl tokio::io::FramedIo for MyTransport {
    type In = Frame<String, Empty, io::Error>;
    type Out = Frame<String, Empty, io::Error>;
    fn poll_read(&mut self) -> Async<()> {
        unimplemented!()
    }
    fn read(&mut self) -> Poll<Self::Out, io::Error> {
        unimplemented!()
    }
    fn poll_write(&mut self) -> Async<()> {
        unimplemented!()
    }
    fn write(&mut self, msg: Self::In) -> Poll<(), io::Error> {
        unimplemented!()
    }
    fn flush(&mut self) -> Poll<(), io::Error> {
        unimplemented!()
    }
}

impl MyClient {
    fn connect(handle: Handle) -> Self {
        let client = pipeline::connect(|| Ok(MyTransport), &handle);
        MyClient {
            handle: handle,
            client: client,
        }
    }
}

impl Service for MyClient {}

pub fn main() {
    env_logger::init().unwrap();

    let mut core = Core::new().unwrap();

    // Now our client. We use the same reactor as for the server - usually though this would be
    // done in a separate program most likely on a separate machine.
    let client = MyClient::connect(core.handle());

    // The connect call returns us a ClientHandle that allows us to use the 'Service' as a function
    // - one that returns a future that we can 'await' on.
    let resp = client.call("Hello".to_string());
    println!("RESPONSE: {:?}", core.run(resp));
}
