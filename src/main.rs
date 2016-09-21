extern crate log;
extern crate env_logger;
extern crate tokio_service;
extern crate futures;
use futures::{Future, Async};
use futures::task;
use futures::task::{Unpark,Spawn};
use futures::stream::{self, Stream, Sender, Receiver};

use tokio_service::Service;
use std::sync::Arc;
use std::collections::VecDeque;

#[derive(Debug)]
struct Unparker;

impl Unpark for Unparker {
    fn unpark(&self) {
        println!("Unpark! {:?}", self);

    }
}

#[derive(Debug)]
enum Error {}
struct MyService;
unsafe impl Send for MyService {}

impl Service for MyService {
    type Request = u64;
    type Response = u64;
    type Error = Error;
    type Future = futures::Finished<Self::Response, Self::Error>;
    fn call(&self, req: u64) -> Self::Future {
        futures::finished(req + 1)
    }
    fn poll_ready(&self) -> Async<()> {
        Async::Ready(())
    }
}

fn main() {
    type Resp = Result<u64, Error>;
    type Reqs = (u64, futures::Complete<Resp>);
    let (s, r) : (Sender<Reqs, Error>, Receiver<Reqs, Error>) = stream::channel();
    let unparker = Arc::new(Unparker);

    let client = futures::finished(s)
        .and_then(|s| {
            let (c, f) = futures::oneshot();
            s.send(Ok((42, c))).map_err(|e| panic!("Send error")).map(|s| (s, f))
        }).and_then(|(s, f)| f)
        .map_err(|futures::Canceled| panic!("Cancelled"))
        .map(|r| println!("Result: {:?}", r));

    let myserv = MyService;
//.then(|r| Ok(k.complete(r)))
    let serv = r
        .and_then(move |(q, k)| { (myserv.call(q), Ok(k))})
        .for_each(|(r, k)| Ok(k.complete(Ok(r))));

    println!("Setup done!");
    let mut tasks = VecDeque::new();

    tasks.push_back(task::spawn(client.boxed()));
    tasks.push_back(task::spawn(serv.boxed()));

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
