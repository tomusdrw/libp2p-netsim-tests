#[macro_use]
extern crate libp2p;
extern crate netsim;
extern crate tokio;
extern crate tokio_core;

use std::{
    io,
    net::Ipv4Addr,
};
use libp2p::{tokio_io, Transport, PeerId};
use libp2p::core::{nodes, transport::boxed::Boxed, StreamMuxer, muxing::StreamMuxerBox};
use libp2p::secio::SecioKeyPair;
use libp2p::futures::{stream, Async, Stream, Future, future::Either};
use libp2p::multiaddr::ToMultiaddr;

mod network;
mod transport;

#[derive(Debug)]
enum NodeKind {
    Sender,
    Receiver,
}

struct Node {
    id: network::NodeId,
    kind: NodeKind,
    swarm: libp2p::core::nodes::swarm::Swarm<
        Boxed<(PeerId, StreamMuxerBox)>,
        StreamMuxerBox,
        (),
    >,
}

impl Node {
    pub fn new(id: network::NodeId, kind: NodeKind, addr: Ipv4Addr) -> Self {
        let id2 = id.clone();

        let key = SecioKeyPair::secp256k1_generated().unwrap();
        let mut swarm = nodes::swarm::Swarm::new(
            transport::build_transport(key)
                        // move |future, _| {
            //     let id = id2.clone();
            //     Box::new(match kind  {
            //         NodeKind::Sender => {
            //             println!("[{}] Sender sending.", id);
            //             Either::A(tokio_io::io::write_all(future, format!("Hello World from: {}", id))
            //                 .map(move |(_more, _res)| {
            //                     println!("[{}] Sent", id);
            //                 }))
            //         },
            //         NodeKind::Receiver => {
            //             println!("[{}] Receiver receiving.", id);
            //             Either::B(tokio_io::io::read_to_end(future, vec![])
            //                 .map(move |(_more, res)| {
            //                     println!("[{}] Received: {:?}", id, String::from_utf8(res));
            //                 }))
            //         },
            //     }) as Box<Future<Item=(), Error=io::Error>>
            // },
        );

        let multiaddr = multiaddr![IP4(addr), TCP(1025u16)];
        let _addr = swarm.listen_on(multiaddr).unwrap();

        Self {
            id,
            kind,
            swarm,
        }
    }
}

impl network::RunningNode for Node {
    type Result = ();

    fn connect_to(&mut self, addr: Ipv4Addr) {
        let multiaddr = multiaddr![IP4(addr), TCP(1025u16)];
        self.swarm.dial(multiaddr).unwrap();
    }

    fn wait(mut self) -> Self::Result {
        let id = self.id.clone();
        let log = move |msg: String| println!("[{}] {}", id, msg);
        log("Running.".into());

        let mut swarm = self.swarm;
        let res = tokio::runtime::current_thread::block_on_all(
            stream::poll_fn(move || {
                loop {
                    match swarm.poll() {
                        Err(e) => return Err(e),
                        Ok(Async::NotReady) => return Ok(Async::NotReady),
                        Ok(Async::Ready(None)) => return Ok(Async::Ready(None)),
                        Ok(Async::Ready(Some(ev))) => match ev {
                            nodes::swarm::SwarmEvent::IncomingConnection { listen_addr } => {
                                log(format!("Incoming {:?}", listen_addr));
                            },
                            nodes::swarm::SwarmEvent::IncomingConnectionError { listen_addr, error } => {
                                log(format!("Incoming Error {:?}: {:?}", listen_addr, error));
                            },
                            nodes::swarm::SwarmEvent::Connected { peer_id, .. } => {
                                log(format!("{:?} Connected", peer_id));
                                if let Some(mut peer) = swarm.peer(peer_id).as_connected() {
                                    peer.open_substream(());
                                }
                            },
                            nodes::swarm::SwarmEvent::OutboundSubstream { peer_id, user_data, substream, .. } => {
                                log(format!("{:?} OutboundStream {:?}", peer_id, user_data));
                            },
                            nodes::swarm::SwarmEvent::InboundSubstream { peer_id, substream, .. } => {
                                log(format!("{:?} InboundStream", peer_id));
                            },
                            nodes::swarm::SwarmEvent::InboundClosed { peer_id, .. } => {
                                log(format!("{:?} InboundClosed", peer_id));
                            },
                            nodes::swarm::SwarmEvent::OutboundClosed { peer_id, user_data, .. } => {
                                log(format!("{:?} OutboundClosed {:?}", peer_id, user_data));
                            },
                            nodes::swarm::SwarmEvent::ListenerClosed { listen_addr, .. } => {
                                log(format!("{:?} ListenerClosed", listen_addr));
                            },
                            nodes::swarm::SwarmEvent::Replaced { peer_id, .. } => {
                                log(format!("{:?} Replaced", peer_id));
                            },
                            nodes::swarm::SwarmEvent::NodeClosed { peer_id, .. } => {
                                log(format!("{:?} NodeClosed", peer_id));
                            },
                            nodes::swarm::SwarmEvent::NodeError { peer_id, error, .. } => {
                                log(format!("{:?} NodeError: {:?}", peer_id, error));
                            },
                            nodes::swarm::SwarmEvent::DialError { peer_id, error, .. } => {
                                log(format!("{:?} DialError: {:?}", peer_id, error));
                            },
                            nodes::swarm::SwarmEvent::UnknownPeerDialError { error, .. } => {
                                log(format!("UnknownPeerDialError: {:?}", error));
                            },
                            _ => {
                                log("Some other event".into());
                            },
                        }
                    }
                }
            }).for_each(|_: Option<()>| Ok(()))
        );
    }
}

fn main() {
    let mut network = network::Network::default();
    let a = network.node("recv1", NodeKind::Receiver);
    // let b = network.node("recv2", NodeKind::Receiver);

    let c = network.node("send1", NodeKind::Sender);
    let d = network.node("send2", NodeKind::Sender);

    // network.connect_all(&[a, b, c, d]);
    network.connect_all(&[a, c, d]);

    network.start(|id, kind, addr| {
        Node::new(id, kind, addr)
    });
}
