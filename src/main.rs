extern crate libp2p;
extern crate netsim;
extern crate tokio_core;
extern crate tokio_current_thread;

use std::{
    io,
    net::Ipv4Addr,
};
use libp2p::{tokio_io, Transport, PeerId};
use libp2p::core::{transport::boxed::Boxed, StreamMuxer, muxing::StreamMuxerBox};
use libp2p::futures::{Stream, Future, future::Either};
use libp2p::multiaddr::ToMultiaddr;

mod network;
// mod transport;

#[derive(Debug)]
enum NodeKind {
    Sender,
    Receiver,
}

struct Node {
    id: network::NodeId,
    controller: libp2p::core::SwarmController<
        libp2p::core::transport::dummy::DummyMuxing<libp2p::CommonTransport>,
        Box<Future<Item = (), Error = io::Error>>
    >,
    run: Box<Future<Item = (), Error = ()>>,
}

impl Node {
    pub fn new(id: network::NodeId, kind: NodeKind, addr: Ipv4Addr) -> Self {
        let transport = libp2p::CommonTransport::new();
        let id2 = id.clone();
        let (controller, run) = libp2p::swarm(
            transport.with_dummy_muxing(),
            move |future, _| {
                let id = id2.clone();
                Box::new(match kind  {
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
                }) as Box<Future<Item=(), Error=io::Error>>
            },
        );

        let mut multiaddr = addr.to_multiaddr().unwrap();
        multiaddr.append(libp2p::multiaddr::AddrComponent::TCP(1025));
        let _addr = controller.listen_on(multiaddr.clone()).unwrap();

        Self {
            id,
            controller,
            run: Box::new(run.for_each(|_| Ok(())).map_err(|e| panic!("{:?}", e))),
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
        tokio_current_thread::block_on_all(self.run).unwrap();
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
