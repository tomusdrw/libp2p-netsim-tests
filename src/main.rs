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
    id: network::NodeId,
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

        let _addr = controller.listen_on(multiaddr.clone()).unwrap();

        Receiver {
            id,
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
        println!("[{}] Receiver running.", self.id);
        let _res = self.core.run(self.run.take().unwrap());

        ()
    }
}

struct Sender {
    id: network::NodeId,
    core: Core,
    transport: libp2p::CommonTransport,
    target: libp2p::Multiaddr,
}

impl Sender {
    pub fn new(id: network::NodeId) -> Self {
        Sender {
            id,
            core: Core::new().unwrap(),
            transport: libp2p::CommonTransport::new(),
            target: "/ip4/127.0.0.1".parse().unwrap(),
        }
    }
}

impl network::RunningNode for Sender {
    type Result = ();

    fn connect_to(&mut self, addr: Ipv4Addr) {
        let mut multiaddr = addr.to_multiaddr().unwrap();
        multiaddr.append(libp2p::multiaddr::AddrComponent::TCP(1025));
        self.target = multiaddr;
    }

    fn wait(mut self) -> Self::Result {
        println!("[{}] Sender running.", self.id);
        ::std::thread::sleep_ms(10);
        let id = self.id.clone();
        let run = self.transport.clone().dial(self.target.clone())
            .unwrap()
            .map(move |(mut stream, _addr)| {
                println!("[{}] Connected.", id);
                stream.write_all(b"Hello World!").unwrap();
                println!("[{}] Wrote data.", id);
            })
            .map_err(|e| println!("{:?}", e));

        let _res = self.core.run(run);
    }
}


fn main() {
    let mut network = network::Network::default();
    let a = network.node("node1", NodeKind::Receiver);
    let b = network.node("node2", NodeKind::Sender);
    let c = network.node("node3", NodeKind::Sender);

    network.connect_all(&[b, a.clone()]);
    network.connect_all(&[c, a]);

    network.start(|id, kind, addr| {
    match kind {
            NodeKind::Sender => {
                Node::Sender(Sender::new(id))
            },
            NodeKind::Receiver => {
                Node::Receiver(Receiver::new(id, addr))
            },
        }
    });
}
