#[macro_use]
extern crate futures;
extern crate backtrace;
extern crate env_logger;
extern crate log;
extern crate tokio_service;
use backtrace::Backtrace;
use futures::stream::{self, Stream, Sender, FutureSender, Receiver};
use futures::task::{self, Unpark, Spawn};
use futures::{Future, IntoFuture, Async, Poll};
use host::Host;
use mpsc::MpscReceiver;
use plex::{PlexClient,PlexService};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::mem;
use std::sync::{Arc, Mutex};
use tokio_service::Service;

mod host;
mod mpsc;
mod plex;
#[derive(Debug)]
pub enum Empty {}


// To simulate a network using the channels here, it's probably best to use
// a hub and spoke architecture (ie: have a switch in the middle that can be
// variably unreliable) and use a (Sender,Receiver) pair as our port.

// Next thing to do I'd reckon is emulate a token ring type thing. First using
// direct connections, and then using the switch component.
//
// Hub: Uses "futures::select_all" to multiplex inputs.

enum SwitchPort<V> {
    Dead,
    Idle(Sender<u64, ()>, Receiver<(usize, u64), ()>),
    Blocked(FutureSender<u64, ()>, Receiver<(usize, u64), ()>),
}
#[derive(Debug)]
struct Switch {
    ports: BTreeMap<usize, SwitchPort>,
    queues: BTreeMap<usize, VecDeque<u64>>,
}

impl Switch {
    fn new() -> Self {
        Switch {
            ports: BTreeMap::new(),
            queues: BTreeMap::new(),
        }
    }

    fn connect(&mut self, name: usize) -> (MpscSender<()>, MpscReceiver<()>) {
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

#[derive(Debug)]
struct Unparker(String, Arc<Mutex<VecDeque<String>>>);

impl Unpark for Unparker {
    fn unpark(&self) {
        println!("Unpark! {:?}: {:?}", self, Backtrace::new() );
        self.1.lock().expect("lock").push_back(self.0.clone());
    }
}

struct AddOne;
unsafe impl Send for AddOne {}

impl Service for AddOne {
    type Request = u64;
    type Response = u64;
    type Error = Empty;
    type Future = futures::Finished<Self::Response, Self::Error>;
    fn call(&self, req: u64) -> Self::Future {
        println!("AddOne#call: {:?}", req);
        futures::finished(req + 1)
    }
    fn poll_ready(&self) -> Async<()> {
        Async::Ready(())
    }
}

const CLIENT_ID: usize = 0;
const PONGER_ID: usize = 1;

fn main() {
    type Resp = Result<u64, Empty>;
    type Reqs = (u64, futures::Complete<Resp>);

    let mut switch = Switch::new();
    let mut tasks: BTreeMap<String, _> = BTreeMap::new();
    let mut scheduled = Arc::new(Mutex::new(VecDeque::<String>::new()));

    let (handle, srv) = Host::build(AddOne);

    let client = PlexClient::<u64, u64, Empty>::new(PONGER_ID, switch.connect(CLIENT_ID));



    let ponger = PlexService::service(AddOne, switch.connect(PONGER_ID));


    let client_f = futures::lazy(move || {
        (client.call(32), client.call(42), client.call(89))
            .into_future()
            .map_err(|e| panic!("{:?}", e))
            .map(|r| println!("Result: {:?}", r))
    });

    for (id, fut) in vec![(format!("Client"), client_f), (format!("Ponger"), ponger)] {
        // Scheduler::add(&mut self, "Client", cient_f);
        let unparker = Arc::new(Unparker(id.clone(), scheduled.clone()));
        tasks.insert(id.clone(), (unparker, task::spawn(client_f.boxed())));
        scheduled.lock().expect("lock").push_back(id);
    }


    // switch.post_delivery(0, 0);

    let swid = format!("Switch");
    let switch_unpark = Arc::new(Unparker(swid.clone(), scheduled.clone()));

    // All of this stuff should be in it's own type, at least.
    tasks.insert(swid.clone(), (switch_unpark, task::spawn(switch.boxed())));

    scheduled.lock().expect("lock").push_back(swid);
    loop {
        let mut pending = mem::replace(&mut *scheduled.lock().expect("lock"), VecDeque::new());
        println!("Pre poll; {:?} after this", pending);
        if pending.is_empty() {
            println!("Nothing scheduled? Try brute force!");
            pending.extend(tasks.keys().cloned());
        }
        for id in pending {
            println!("Poke task: {:?}", id);
            let &mut (ref mut unparker, ref mut t) = tasks.get_mut(&id).expect("task");
            match t.poll_future(unparker.clone()) {
                Ok(Async::Ready(v)) => {
                    println!("done: {:?}", v);
                }
                Ok(Async::NotReady) => {
                    println!("pending:");
                    // scheduled.lock().expect("lock").push_back(id);
                }
                Err(e) => println!("Error: {:?}", e),
            }
        }
    }
}
