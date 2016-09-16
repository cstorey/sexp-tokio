extern crate log;
extern crate env_logger;
#[macro_use]
extern crate futures;
use futures::{Future, Async, Poll};
use futures::task;
use futures::task::Unpark;
use futures::stream::{self, Stream, Sender, Receiver};

use std::sync::Arc;
use std::fmt;
use std::collections::{BTreeMap, VecDeque};
use std::mem;

#[derive(Debug)]
struct Unparker;

impl Unpark for Unparker {
    fn unpark(&self) {
        println!("Unpark! {:?}", self);

    }
}


// To simulate a network using the channels here, it's probably best to use
// a hub and spoke architecture (ie: have a switch in the middle that can be
// variably unreliable) and use a (Sender,Receiver) pair as our port.

// Next thing to do I'd reckon is emulate a token ring type thing. First using
// direct connections, and then using the switch component.
//
// Hub: Uses "futures::select_all" to multiplex inputs.

enum SwitchPort {
    Dead,
    Idle(Sender<usize, ()>, Receiver<usize, ()>, VecDeque<u64>),
}
#[derive(Debug)]
struct Switch {
    ports: BTreeMap<usize, SwitchPort>,
}

struct Connection(Sender<usize, ()>, Receiver<usize, ()>);

impl Switch {
    fn new() -> Self {
        Switch { ports: BTreeMap::new() }
    }

    fn connect(&mut self, name: u64) -> Connection {
        // switch -> member
        let (smtx, smrx) = stream::channel();
        // member -> switch
        let (mstx, msrx) = stream::channel();

        Connection(mstx, smrx)
    }

    fn post_delivery(&mut self, dest: usize, val: u64) {
        let port = self.ports.get_mut(&dest).expect("post_delivery: port");
        port.enqueue(val);
    }
}

impl Future for Switch {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Poll<(), ()> {
        println!("Poll: {:?}", self);
        for (n, port) in self.ports.iter_mut() {
            match self {
                &mut SwitchPort::Idle(_, _, ref mut queue) => queue.push_back(val),
            }
        }
        Ok(Async::Ready(()))
    }
}

impl SwitchPort {
    fn enqueue(&mut self, val: u64) {
        match self {
            &Member::Dead => panic!("enqueue to dead port"),
            &mut SwitchPort::Idle(_, _, ref mut queue) => queue.push_back(val),
        }
    }
}

impl fmt::Debug for SwitchPort {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Member::Dead => fmt.debug_tuple("Dead").finish(),
            &SwitchPort::Idle(_, _, ref queue) => fmt.debug_tuple("Idle").field(&queue).finish(),
        }
    }
}
enum Member {
    Dead,
    Idle(Connection, u64),
}

impl fmt::Debug for Member {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Member::Dead => fmt.debug_tuple("Dead").finish(),
            &Member::Idle(_, dest) => fmt.debug_tuple("Idle").field(&dest).finish(),
        }
    }
}

impl Member {
    fn new(conn: Connection, dest: u64) -> Member {
        Member::Idle(conn, dest)
    }
}

impl Future for Member {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Poll<(), ()> {
        println!("Poll: {:?}", self);
        match mem::replace(self, Member::Dead) {
            Member::Idle(Connection(tx, mut rx), dest) => {
                match try_ready!(rx.poll()) {
                    Some(val) => {
                        // Some stuff
                    }
                    None => {
                        // No-op
                    }
                }
            }
            Member::Dead => (),
        }
        Ok(Async::Ready(()))
    }
}

fn main() {

    let mut switch = Switch::new();
    let mut tasks = VecDeque::new();

    for n in 0..4 {
        let conn = switch.connect(n);

        let member = Member::new(conn, (n + 1) % 4);

        tasks.push_back(task::spawn(member.boxed()));
    }

    switch.post_delivery(0, 0);

    tasks.push_back(task::spawn(switch.boxed()));

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
