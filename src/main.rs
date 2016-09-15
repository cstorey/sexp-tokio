extern crate log;
extern crate env_logger;
extern crate futures;
use futures::{Future, Async};
use futures::task;
use futures::task::Unpark;
use futures::stream::{self, Stream, Sender, Receiver};

use std::sync::Arc;
use std::collections::VecDeque;

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


    let input = (0..10).map(Ok).collect::<Vec<Result<usize, ()>>>();
    let inp = stream::iter(input.into_iter());
    let sfut = inp.fold(s, |s, n| {
                      println!("Send: {:?}", n);
                      s.send(Ok(n)).map_err(|_e| println!("Sender error!"))
                  })
                  .and_then(|_| Ok(()));
    println!("Setup done!");

    let mut tasks = VecDeque::new();

    tasks.push_back(task::spawn(sfut.boxed()));
    tasks.push_back(task::spawn(serv.boxed()));

    let unparker = Arc::new(Unparker);
    while let Some(mut t) = tasks.pop_front() {
        println!("Pre poll");
        match t.poll_future(unparker.clone()) {
            Ok(Async::Ready(v)) => {
                println!("done: {:?}", v);
                break;
            }
            Ok(Async::NotReady) => {
                println!("pending:");
                tasks.push_back(t);
            }
            Err(e) => println!("Error: {:?}", e),
        }
    }

}
