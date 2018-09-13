extern crate netsim;
extern crate tokio_core;
extern crate libp2p;

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
    id: network::NodeId,
    core: Core,
    controller: libp2p::core::SwarmController<libp2p::core::transport::dummy::DummyMuxing<libp2p::CommonTransport>>,
    run: Box<Future<Item = (), Error = ()>>,
}

impl Receiver {
    pub fn new(id: network::NodeId, addr: Ipv4Addr) -> Self {
        let core = Core::new().unwrap();
        let transport = libp2p::CommonTransport::new();
        let id2 = id.clone();
        let (controller, run) = libp2p::swarm(
            transport.with_dummy_muxing(),
            move |future, _| {
                let id = id2.clone();
                tokio_io::io::read_to_end(future, vec![])
                    .map(move |(_more, res)| {
                        println!("[{}] Received: {:?}", id, String::from_utf8(res));
                    })
            },
        );
        let mut multiaddr = addr.to_multiaddr().unwrap();
        multiaddr.append(libp2p::multiaddr::AddrComponent::TCP(1025));
        let _addr = controller.listen_on(multiaddr.clone()).unwrap();

        Receiver {
            id,
            core,
            controller,
            run: Box::new(run.map_err(|e| panic!("{:?}", e))),
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
        println!("[{}] Receiver running.", self.id);
        let _res = self.core.run(self.run);

        ()
    }
}

struct Sender {
    id: network::NodeId,
    core: Core,
    controller: libp2p::core::SwarmController<libp2p::core::transport::dummy::DummyMuxing<libp2p::CommonTransport>>,
    run: Box<Future<Item = (), Error = ()>>,
}

impl Sender {
    pub fn new(id: network::NodeId, addr: Ipv4Addr) -> Self {
        let transport = libp2p::CommonTransport::new();
        let id2 = id.clone();
        let (controller, run) = libp2p::swarm(
            transport.with_dummy_muxing(),
            move |future, _| {
                let id = id2.clone();
                println!("[{}] Sender sending.", id);
                tokio_io::io::write_all(future, "Hello World!")
                    .map(move |(_more, _res)| {
                        println!("[{}] Sent", id);
                    })
            },
        );

        let mut multiaddr = addr.to_multiaddr().unwrap();
        multiaddr.append(libp2p::multiaddr::AddrComponent::TCP(1025));
        let _addr = controller.listen_on(multiaddr.clone()).unwrap();

        Sender {
            id,
            core: Core::new().unwrap(),
            controller,
            run: Box::new(run.map_err(|e| panic!("{:?}", e))),
        }
    }
}

impl network::RunningNode for Sender {
    type Result = ();

    fn connect_to(&mut self, addr: Ipv4Addr) {
        let mut multiaddr = addr.to_multiaddr().unwrap();
        multiaddr.append(libp2p::multiaddr::AddrComponent::TCP(1025));
        self.controller.dial(multiaddr, libp2p::CommonTransport::new()).unwrap();
    }

    fn wait(mut self) -> Self::Result {
        println!("[{}] Sender running.", self.id);
        let _res = self.core.run(self.run);

        ()
    }
}


fn main() {
    let mut network = network::Network::default();
    let a = network.node("node1", NodeKind::Receiver);
    let b = network.node("node2", NodeKind::Sender);
    let c = network.node("node3", NodeKind::Sender);

    network.connect_all(&[a.clone(), b]);
    network.connect_all(&[a, c]);

    network.start(|id, kind, addr| {
    match kind {
            NodeKind::Sender => {
                Node::Sender(Sender::new(id, addr))
            },
            NodeKind::Receiver => {
                Node::Receiver(Receiver::new(id, addr))
            },
        }
    });
}
