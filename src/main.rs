extern crate netsim;
extern crate tokio_core;
extern crate libp2p;

use std::io::{Read, Write};
use std::net::UdpSocket;
use tokio_core::reactor::Core;
use netsim::{spawn, node, Network, Ipv4Range};
use libp2p::{tokio_io, Transport};
use libp2p::futures::{Future, IntoFuture};
use libp2p::multiaddr::ToMultiaddr;

fn main() {
    // Create an event loop and a network to bind devices to.
    let mut core = Core::new().unwrap();
    let network = Network::new(&core.handle());

    let (tx, rx) = std::sync::mpsc::channel();

    let receiver_node = node::ipv4::machine(move |ipv4_addr| {
        let mut core = Core::new().unwrap();
        let transport = libp2p::CommonTransport::new();

        let (controller, run) = libp2p::swarm(
            transport.with_dummy_muxing(),
            |future, _| {
                tokio_io::io::read_to_end(future, vec![])
                    .map(|(_more, res)| {
                        println!("[node1] Received: {:?}", String::from_utf8(res));
                    })
            },
        );

        let mut multiaddr = ipv4_addr.to_multiaddr().unwrap();
        multiaddr.append(libp2p::multiaddr::AddrComponent::TCP(1025));
        let addr = controller.listen_on(multiaddr.clone()).unwrap();
        println!("[node1] Listening on {:?}", addr);
        tx.send(multiaddr).unwrap();

        core.run(run)
    });

    // Create the machine which will send the UDP packet
    let sender_node = node::ipv4::machine(move |_ipv4_addr| {
        let mut core = Core::new().unwrap();
        let receiver_multiaddr = rx.recv().unwrap();
        let transport = libp2p::CommonTransport::new();
        let connection = transport.dial(receiver_multiaddr).unwrap();
        core.run(connection.map(|(mut stream, _addr)| {
            println!("[node2] Connected.");
            stream.write_all(b"Hello World!").unwrap();
            println!("[node2] Wrote data.");
        }))
    });

    // Connect the sending and receiving nodes via a router
    let router_node = node::ipv4::router((receiver_node, sender_node));

    // Run the network with the router as the top-most node. `_plug` could be used send/receive
    // packets from/to outside the network
    let (spawn_complete, _plug) = spawn::ipv4_tree(&network.handle(), Ipv4Range::global(), router_node);

    // Drive the network on the event loop and get the data returned by the receiving node.
    let _result = core.run(spawn_complete).unwrap();
    // assert_eq!(&received[..], b"hello world");
}
