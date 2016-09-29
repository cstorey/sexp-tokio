extern crate futures;
extern crate tokio_core as tokio;
extern crate tokio_service as service;
extern crate miniature_goggles as goggles;
extern crate env_logger;

use tokio::reactor::Core;
use service::Service;

pub fn main() {
    env_logger::init().unwrap();

    let mut core = Core::new().unwrap();

    // This brings up our server.
    // let addr = "127.0.0.1:12345".parse().unwrap();

    // let service = goggles::service::serve(&core.handle(),
    // addr,
    // service::simple_service(|msg| {
    // println!("GOT: {:?}", msg);
    // Ok(msg)
    // }));
    //
    // core.run(futures::empty::<(), ()>());
}
