extern crate log;
extern crate env_logger;
extern crate futures;
use futures::Future;
use futures::task;
use futures::task::Unpark;

use std::sync::Arc;

#[derive(Debug)]
struct Unparker;

impl Unpark for Unparker {
    fn unpark(&self) {
        println!("Unpark! {:?}", self);

    }
}

fn main() {

    let (mut c, mut p) = futures::oneshot();
    let mut t = task::spawn(p);
    let mut unparker = Arc::new(Unparker);

    println!("Setup done!");
    println!("Poll: {:?}", t.poll_future(unparker.clone()));

    println!("Pre complete");
    c.complete(42);
    println!("Post complete");
    println!("Poll: {:?}", t.poll_future(unparker.clone()));
}
