extern crate log;
extern crate env_logger;
#[macro_use]
extern crate futures;
use futures::{Future, Async, Poll};
use futures::task;
use futures::task::Unpark;
use futures::stream::{self, Stream, Sender, FutureSender, Receiver};

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
    Idle(Sender<u64, ()>, Receiver<(usize, u64), ()>),
    Blocked(FutureSender<u64, ()>, Receiver<(usize, u64), ()>),
}
#[derive(Debug)]
struct Switch {
    ports: BTreeMap<usize, SwitchPort>,
    queues: BTreeMap<usize, VecDeque<u64>>,
}

struct Connection(Sender<(usize, u64), ()>, Receiver<u64, ()>);

impl Switch {
    fn new() -> Self {
        Switch {
            ports: BTreeMap::new(),
            queues: BTreeMap::new(),
        }
    }

    fn connect(&mut self, name: usize) -> Connection {
        // switch -> member
        let (smtx, smrx) = stream::channel();
        // member -> switch
        let (mstx, msrx) = stream::channel();

        let our_side = SwitchPort::Idle(smtx, msrx);
        self.ports.insert(name, our_side);
        self.queues.insert(name, VecDeque::new());

        Connection(mstx, smrx)
    }

    fn post_delivery(&mut self, dest: usize, val: u64) {
        if let Some(q) = self.queues.get_mut(&dest) {
            q.push_back(val);
        } else {
            panic!("Attempted to empty port: {:?}", dest);
        }
    }
}

impl Future for Switch {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Poll<(), ()> {
        println!("Switch::poll pre: {:?}", self);
        {
            let &mut Switch { ref mut queues, ref mut ports } = self;
            for (n, port) in ports.iter_mut() {
                let mut newport = mem::replace(port, SwitchPort::Dead);

                if let SwitchPort::Dead = newport {
                    panic!("poll dead port");
                };
                if let SwitchPort::Idle(tx, mut rx) = newport {
                    newport = if let Some(msg) = queues.get_mut(n).and_then(|q| q.pop_front()) {
                        let txfut = tx.send(Ok(msg));
                        SwitchPort::Blocked(txfut, rx)
                    } else {
                        match rx.poll() {
                            Ok(Async::Ready(Some((dst, val)))) => {
                                queues.get_mut(&dst).expect("port queue").push_back(val);
                                SwitchPort::Idle(tx, rx)
                            }
                            Ok(Async::Ready(None)) => {
                                println!("SwitchPort: {:?} Disconnected !", n);
                                SwitchPort::Dead
                            }
                            Ok(Async::NotReady) => SwitchPort::Idle(tx, rx),
                            Err(e) => {
                                println!("SwitchPort rx poll: {:?}", e);
                                return Err(e);
                            }
                        }
                    }

                    // match rx.poll() {
                    // Ok(Async::Ready(Some(val))) => {
                    // println!("Got val: {:?} for {:?}", val, dest);
                    // Member::Blocked(tx.send(Ok((dest, val))), rx, dest)
                    // }
                    // }
                    //
                };

                if let SwitchPort::Blocked(mut txfut, rx) = newport {
                    newport = match txfut.poll() {
                        Ok(Async::Ready(tx)) => SwitchPort::Idle(tx, rx),
                        Ok(Async::NotReady) => SwitchPort::Blocked(txfut, rx),
                        Err(e) => {
                            println!("Send Error on port {:?}!", n);
                            return Err(());
                        }
                    }
                };
                *port = newport;
            }

            // let txfut = tx.send(Ok(val));
            // SwitchPort::Blocked(txfut, rx, queue)
            //
        }
        println!("Switch::poll post: {:?}", self);
        if self.ports.is_empty() {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::NotReady)
        }

    }
}

impl fmt::Debug for SwitchPort {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &SwitchPort::Dead => fmt.debug_tuple("Dead").finish(),
            &SwitchPort::Idle(_, _) => fmt.debug_tuple("Idle").finish(),
            &SwitchPort::Blocked(_, _) => fmt.debug_tuple("Blocked").finish(),
        }
    }
}
enum Member {
    Dead,
    Idle(Sender<(usize, u64), ()>, Receiver<u64, ()>, usize),
    Blocked(FutureSender<(usize, u64), ()>, Receiver<u64, ()>, usize),
}

impl fmt::Debug for Member {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Member::Dead => fmt.debug_tuple("Dead").finish(),
            &Member::Idle(_, _, dest) => fmt.debug_tuple("Idle").field(&dest).finish(),
            &Member::Blocked(_, _, dest) => fmt.debug_tuple("Blocked").field(&dest).finish(),
        }
    }
}

impl Member {
    fn new(Connection(tx, rx): Connection, dest: usize) -> Member {
        Member::Idle(tx, rx, dest)
    }
}

impl Future for Member {
    type Item = ();
    type Error = ();
    fn poll(&mut self) -> Poll<(), ()> {
        println!("Member::poll: {:?}", self);
        let mut this = mem::replace(self, Member::Dead);
        if let Member::Idle(tx, mut rx, dest) = this {
            this = match rx.poll() {
                Ok(Async::Ready(Some(val))) => {
                    println!("Got val: {:?} for {:?}", val, dest);
                    Member::Blocked(tx.send(Ok((dest, val + 1))), rx, dest)
                }
                Ok(Async::Ready(None)) => {
                    println!("Disconnected! dest:{:?}", dest);
                    Member::Dead
                }
                Ok(Async::NotReady) => Member::Idle(tx, rx, dest),
                Err(e) => {
                    println!("Member rx poll: {:?}", e);
                    return Err(e);
                }
            };
        };
        if let Member::Blocked(mut txfut, rx, dest) = this {
            this = match txfut.poll() {
                Ok(Async::Ready(tx)) => {
                    println!("RTS");
                    Member::Idle(tx, rx, dest)
                }
                Ok(Async::NotReady) => Member::Blocked(txfut, rx, dest),
                Err(_) => {
                    println!("Send Error to dest {:?}!", dest);
                    return Err(());
                }
            }
        }
        *self = this;
        println!("Member::poll post: {:?}", self);
        if let &Member::Dead = &*self {
            Ok(Async::Ready(()))
        } else {
            Ok(Async::NotReady)
        }
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
        println!("Pre poll; {:?} after this", tasks.len());
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
