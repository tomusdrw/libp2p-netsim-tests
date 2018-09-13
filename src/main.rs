extern crate netsim;
extern crate tokio_core;
extern crate libp2p;

use std::io::{Write};
use std::net::Ipv4Addr;
use tokio_core::reactor::Core;
use libp2p::{tokio_io, Transport};
use libp2p::futures::{Future};
use libp2p::multiaddr::ToMultiaddr;

mod network;

#[derive(Debug)]
enum NodeKind {
    Sender,
    Receiver,
}

enum Node {
    Sender(Sender),
    Receiver(Receiver),
}

impl network::RunningNode for Node {
    type Result = ();

    fn connect_to(&mut self, addr: Ipv4Addr) {
        match *self {
            Node::Sender(ref mut sender) => sender.connect_to(addr),
            Node::Receiver(ref mut recv) => recv.connect_to(addr),
        }
    }

    fn wait(self) -> Self::Result {
        match self {
            Node::Sender(sender) => sender.wait(),
            Node::Receiver(recv) => recv.wait(),
        }
    }
}

struct Receiver {
    core: Core,
    controller: libp2p::core::SwarmController<libp2p::core::transport::dummy::DummyMuxing<libp2p::CommonTransport>>,
    run: Option<Box<Future<Item = (), Error = ()>>>,
}

impl Receiver {
    pub fn new(id: network::NodeId, addr: Ipv4Addr) -> Self {
        let core = Core::new().unwrap();
        let transport = libp2p::CommonTransport::new();
        let mut multiaddr = addr.to_multiaddr().unwrap();
        multiaddr.append(libp2p::multiaddr::AddrComponent::TCP(1025));
        let (controller, run) = libp2p::swarm(
            transport.with_dummy_muxing(),
            move |future, _| {
                let id = id.clone();
                tokio_io::io::read_to_end(future, vec![])
                    .map(move |(_more, res)| {
                        println!("[{}] Received: {:?}", id, String::from_utf8(res));
                    })
            },
        );

        let _addr = controller.listen_on(multiaddr.clone()).unwrap();

        Receiver {
            core,
            controller,
            run: Some(Box::new(run.map_err(|e| panic!("{:?}", e)))),
        }
    }
}

impl network::RunningNode for Receiver {
    type Result = ();

    fn connect_to(&mut self, addr: Ipv4Addr) {
        let mut multiaddr = addr.to_multiaddr().unwrap();
        multiaddr.append(libp2p::multiaddr::AddrComponent::TCP(1025));
        self.controller.dial(multiaddr, libp2p::CommonTransport::new()).unwrap();
    }

    fn wait(mut self) -> Self::Result {
        let _res = self.core.run(self.run.take().unwrap());

        ()
    }
}

struct Sender {
    core: Core,
    transport: Option<libp2p::CommonTransport>,
    run: Option<Box<Future<Item = (), Error = ()>>>,
}

impl Sender {
    pub fn new(_id: network::NodeId) -> Self {
        Sender {
            core: Core::new().unwrap(),
            transport: Some(libp2p::CommonTransport::new()),
            run: None,
        }
    }
}

impl network::RunningNode for Sender {
    type Result = ();

    fn connect_to(&mut self, addr: Ipv4Addr) {
        let mut multiaddr = addr.to_multiaddr().unwrap();
        multiaddr.append(libp2p::multiaddr::AddrComponent::TCP(1025));
        let transport = self.transport.take().unwrap();
        self.run = Some(Box::new(transport.dial(multiaddr)
            .unwrap()
            .map(|(mut stream, _addr)| {
                println!("[node2] Connected.");
                stream.write_all(b"Hello World!").unwrap();
                println!("[node2] Wrote data.");
            })
            .map_err(|e| panic!("{:?}", e))
        ));
    }

    fn wait(mut self) -> Self::Result {
        let _res = self.core.run(self.run.take().unwrap());

        ()
    }
}


fn main() {
    let mut network = network::Network::default();
    let a = network.node("node1", NodeKind::Sender);
    let b = network.node("node2", NodeKind::Receiver);

    network.connect_all(&[b, a]);

    network.start(|id, kind, addr| {
    match kind {
            NodeKind::Receiver => {
                Node::Sender(Sender::new(id))
            },
            NodeKind::Sender => {
                Node::Receiver(Receiver::new(id, addr))
            },
        }
    });
}
