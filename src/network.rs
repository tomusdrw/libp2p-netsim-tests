use std::{
    collections::HashMap,
    fmt,
    net::Ipv4Addr,
    sync::mpsc,
};
use tokio_core::reactor::Core;
use netsim::{self, spawn, node, Ipv4Range};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(String);

impl fmt::Display for NodeId {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node<NodeKind> {
    id: NodeId,
    kind: NodeKind,
}

#[derive(Debug)]
pub struct Network<NodeKind> {
    nodes:  HashMap<NodeId, Node<NodeKind>>,
    connections: Vec<
        (NodeId, NodeId),
    >
}

impl<NodeKind> Default for Network<NodeKind> {
    fn default() -> Self {
        Network {
            nodes: Default::default(),
            connections: Default::default(),
        }
    }
}

impl<NodeKind> Network<NodeKind> where
    NodeKind: Send + 'static,
{
    /// Add a new node to the network.
    pub fn node<T: Into<String>>(&mut self, id: T, kind: NodeKind) -> NodeId {
        let id = NodeId(id.into());
        let node = Node {
            id: id.clone(),
            kind,
        };
        let prev = self.nodes.insert(id.clone(), node);
        assert!(prev.is_none(), "Duplicate id: {}", id);

        id
    }

    /// Connects all nodes between each other.
    ///
    /// NOTE opens connections from A->B and also B->A,
    /// but never A->A
    pub fn connect_all(&mut self, nodes: &[NodeId]) {
        for a in nodes {
            for b in nodes {
                if a != b {
                    self.connections.push((a.clone(), b.clone()));
                }
            }
        }
    }

    /// Connect each of the given nodes to the node passed as second parameter.
    pub fn connect_each_to(&mut self, nodes: &[NodeId], target: NodeId) {
        for a in nodes {
            self.connections.push((a.clone(), target.clone()));
        }
    }

    /// Connect a node given as first parameter to all listed nodes.
    pub fn connect_to_all(&mut self, node: NodeId, targets: &[NodeId]) {
        for a in targets {
            self.connections.push((node.clone(), a.clone()));
        }
    }

    pub fn start<R, F, N>(self, node_runner: F) -> HashMap<NodeId, R>
    where
        F: Fn(NodeId, NodeKind, Ipv4Addr) -> N + Send + Clone + 'static,
        N: RunningNode<Result=R>,
        R: Send + 'static
    {
        let mut core = Core::new().unwrap();
        let network = netsim::Network::new(&core.handle());


        let mut nodes = vec![];
        let mut init = vec![];
        let mut addresses = HashMap::new();
        let mut connect_to = HashMap::new();

        for (id, node) in self.nodes {
            let (addr_tx, addr_rx) = mpsc::channel();
            let (conn_tx, conn_rx) = mpsc::channel();
            let (init_tx, init_rx) = mpsc::channel();
            addresses.insert(id.clone(), addr_rx);
            connect_to.insert(id.clone(), conn_tx);
            init.push(init_rx);
            let node_runner = node_runner.clone();

            nodes.push(node::ipv4::machine(move |addr| {
                println!("[{}] Starting", id);

                addr_tx.send(addr).expect("Network not running");
                let mut node = node_runner(node.id, node.kind, addr);

                println!("[{}] Sending start signal", id);
                init_tx.send(()).expect("Network init failed.");

                println!("[{}] Waiting for connections", id);
                for addr in conn_rx {
                    node.connect_to(addr);
                }

                let res = (id.clone(), node.wait());
                println!("[{}] Done", id);
                res
            }));
        }

        // Connect the sending and receiving nodes via a router
        let router_node = node::ipv4::router(nodes);

        // Run the network with the router as the top-most node. `_plug` could be used send/receive
        // packets from/to outside the network
        let (spawn_complete, _plug) = spawn::ipv4_tree(&network.handle(), Ipv4Range::global(), router_node);


        // Make sure we collect the addresses an connect to each other.
        let mut addr = HashMap::new();
        for (id, addr_rx) in addresses {
            match addr_rx.recv() {
                Ok(a) => {
                    addr.insert(id, a);
                },
                Err(e) => {
                    println!("Unable to get address of {}: {:?}", id, e);
                },
            }
        }

        // synchronize start of all nodes
        for a in init {
            a.recv().expect("Node should be sending start signal.");
        }

        // connect to each other
        for (a, b) in self.connections {
            match (connect_to.get(&a), addr.get(&b)) {
                (Some(tx), Some(addr)) => {
                    println!("Connecting {} -> {}", a, b);
                    tx.send(addr.clone()).expect("Node should be listening for connections.");
                },
                _ => {},
            }
        }
        // make sure to drop tx ends of connect_to channels.
        drop(connect_to);

        // Drive the network on the event loop and get the data returned by the receiving node.
        let res = core.run(spawn_complete).unwrap();
        res.into_iter().collect()
    }
}

pub trait RunningNode {
    type Result;

    /// Attempt to connect to a particular address.
    fn connect_to(&mut self, addr: Ipv4Addr);

    /// Wait for this node to run to completion and return a result.
    fn wait(self) -> Self::Result;
}
