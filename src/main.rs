extern crate log;
extern crate env_logger;
extern crate futures;
use futures::{Future, Async};
use futures::task;
use futures::task::Unpark;
use futures::stream::{self, Stream, Sender, Receiver};

use std::sync::Arc;

#[derive(Debug)]
struct Unparker;

impl Unpark for Unparker {
    fn unpark(&self) {
        println!("Unpark! {:?}", self);

    }
}

fn main() {

    let (s, r): (Sender<usize, ()>, Receiver<usize, ()>) = stream::channel();
    let serv = r.for_each(|x| Ok(println!("Recv: {:?}", x)));
    let unparker = Arc::new(Unparker);


    let sfut = futures::finished(s)
                    .and_then(|s| { println!("Send: {:?}", 1); s.send(Ok(1)) })
                    .and_then(|s| { println!("Send: {:?}", 2); s.send(Ok(2)) })
                    .and_then(|s| { println!("Send: {:?}", 3); s.send(Ok(3)) })
                    .map(|_| ())
                    .map_err(|_e| println!("Sender error!"));
    let mut t = task::spawn(sfut.join(serv));
    println!("Setup done!");

    loop {
        println!("Pre poll");
        match t.poll_future(unparker.clone()) {
            Ok(Async::Ready(v)) => {
                println!("done: {:?}", v);
                break;
            }
            Ok(Async::NotReady) => println!("pending:"),
            Err(e) => println!("Error: {:?}", e),
        }
    }
}
