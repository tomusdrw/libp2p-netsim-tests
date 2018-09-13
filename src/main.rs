extern crate netsim;
extern crate tokio_core;
extern crate libp2p;

use std::net::Ipv4Addr;
use tokio_core::reactor::Core;
use libp2p::{tokio_io, Transport};
use libp2p::futures::{Future, future::Either};
use libp2p::multiaddr::ToMultiaddr;

mod network;

#[derive(Debug)]
enum NodeKind {
    Sender,
    Receiver,
}

struct Node {
    id: network::NodeId,
    core: Core,
    controller: libp2p::core::SwarmController<libp2p::core::transport::dummy::DummyMuxing<libp2p::CommonTransport>>,
    run: Box<Future<Item = (), Error = ()>>,
}

impl Node {
    pub fn new(id: network::NodeId, kind: NodeKind, addr: Ipv4Addr) -> Self {
        let core = Core::new().unwrap();
        let transport = libp2p::CommonTransport::new();
        let id2 = id.clone();
        let (controller, run) = libp2p::swarm(
            transport.with_dummy_muxing(),
            move |future, _| {
                let id = id2.clone();
                match kind  {
                    NodeKind::Sender => {
                        println!("[{}] Sender sending.", id);
                        Either::A(tokio_io::io::write_all(future, format!("Hello World from: {}", id))
                            .map(move |(_more, _res)| {
                                println!("[{}] Sent", id);
                            }))
                    },
                    NodeKind::Receiver => {
                        println!("[{}] Receiver receiving.", id);
                        Either::B(tokio_io::io::read_to_end(future, vec![])
                            .map(move |(_more, res)| {
                                println!("[{}] Received: {:?}", id, String::from_utf8(res));
                            }))
                    },
                }
            },
        );

        let mut multiaddr = addr.to_multiaddr().unwrap();
        multiaddr.append(libp2p::multiaddr::AddrComponent::TCP(1025));
        let _addr = controller.listen_on(multiaddr.clone()).unwrap();

        Self {
            id,
            core,
            controller,
            run: Box::new(run.map_err(|e| panic!("{:?}", e))),
        }
    }
}

impl network::RunningNode for Node {
    type Result = ();

    fn connect_to(&mut self, addr: Ipv4Addr) {
        let mut multiaddr = addr.to_multiaddr().unwrap();
        multiaddr.append(libp2p::multiaddr::AddrComponent::TCP(1025));
        self.controller.dial(multiaddr, libp2p::CommonTransport::new()).unwrap();
    }

    fn wait(mut self) -> Self::Result {
        println!("[{}] Running.", self.id);
        let _res = self.core.run(self.run);
        println!("[{}] Done.", self.id);
        ()
    }
}

fn main() {
    let mut network = network::Network::default();
    let a = network.node("recv1", NodeKind::Receiver);
    let b = network.node("recv2", NodeKind::Receiver);

    let c = network.node("send1", NodeKind::Sender);
    let d = network.node("send2", NodeKind::Sender);

    network.connect_all(&[a, b, c, d]);

    network.start(|id, kind, addr| {
        Node::new(id, kind, addr)
    });
}
