extern crate log;
extern crate env_logger;
extern crate tokio_service;
#[macro_use]
extern crate futures;
use futures::{Future, IntoFuture, Async};
use futures::task;
use futures::task::{Unpark, Spawn};
use futures::stream::{self, Stream};

use tokio_service::Service;
use std::sync::Arc;
use std::mem;
use std::fmt;
use std::collections::VecDeque;

mod mpsc;
mod host;

use mpsc::MpscReceiver;
use host::Host;

#[derive(Debug)]
pub enum Empty {}

#[derive(Debug)]
struct Unparker;

impl Unpark for Unparker {
    fn unpark(&self) {
        println!("Unpark! {:?}", self);

    }
}

struct MyService;
unsafe impl Send for MyService {}

impl Service for MyService {
    type Request = u64;
    type Response = u64;
    type Error = Empty;
    type Future = futures::Finished<Self::Response, Self::Error>;
    fn call(&self, req: u64) -> Self::Future {
        println!("MyService#call: {:?}", req);
        futures::finished(req + 1)
    }
    fn poll_ready(&self) -> Async<()> {
        Async::Ready(())
    }
}


fn main() {
    type Resp = Result<u64, Empty>;
    type Reqs = (u64, futures::Complete<Resp>);

    let (client, srv) = Host::build(MyService);

    let unparker = Arc::new(Unparker);

    println!("Setup done!");
    let mut tasks: VecDeque<Spawn<Box<Future<Item = (), Error = Empty>>>> = VecDeque::new();

    let f = futures::lazy(move || {
        (client.call(32), client.call(42), client.call(89))
            .into_future()
            .map_err(|e| panic!("{:?}", e))
            .map(|r| println!("Result: {:?}", r))
    });

    tasks.push_back(task::spawn(f.boxed()));
    tasks.push_back(task::spawn(srv.boxed()));

    let unparker = Arc::new(Unparker);
    while let Some(mut t) = tasks.pop_front() {
        println!("Pre poll");
        match t.poll_future(unparker.clone()) {
            Ok(Async::Ready(v)) => {
                println!("done: {:?}", v);
            }
            Ok(Async::NotReady) => {
                println!("pending:");
                tasks.push_back(t);
            }
            Err(e) => println!("Error: {:?}", e),
        }
    }
}
