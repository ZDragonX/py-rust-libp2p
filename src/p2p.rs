

use futures::stream::StreamExt;
use libp2p::{
    self, gossipsub, identity, kad::{self, store::MemoryStore, Mode, ProgressStep}, mdns::{self},
    noise, swarm::{NetworkBehaviour, Swarm, SwarmEvent}, tcp, yamux, Multiaddr, PeerId, identify, multiaddr::{ Protocol},
};
use std::{collections::HashSet, error::Error, sync::Arc};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use libp2p::gossipsub::Topic;
use tokio::{io, select};
use tokio::sync::{mpsc, RwLock};
use anyhow::Result;
use anyhow::Context;
use tokio::sync::oneshot;
use tokio::sync::broadcast;
use tokio::time::{interval, Duration};
use std::str::FromStr;
use libp2p::core::ConnectedPoint;
use pyo3::prelude::*;
use std::{fs::File, io::Write, io::Read, path::Path};
use crate::file_tools;

#[derive(NetworkBehaviour)]
struct MyBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
    kademlia: kad::Behaviour<MemoryStore>,
    identify: identify::Behaviour,
}

enum SwarmCommand {
    ConnectPeer(Multiaddr, oneshot::Sender<std::result::Result<(), anyhow::Error>>),
    PublishMessage(Vec<u8>, oneshot::Sender<std::result::Result<(), anyhow::Error>>),
    GetConnectedPeers(oneshot::Sender<std::result::Result<HashSet<PeerId>, anyhow::Error>>),
    GetClosestPeers(PeerId),
}

#[derive(Clone, Debug)]
pub struct MessageEvent {
    pub source: PeerId,
    pub content: Vec<u8>,

}

#[pyclass]
#[derive(Clone, Debug)]
pub struct P2PNetwork {
    pub peer_id: PeerId,
    command_tx: mpsc::Sender<SwarmCommand>,
    subscribe_message_tx: broadcast::Sender<MessageEvent>,
    pub full_addrs: Arc<RwLock<Vec<String>>>
}

impl P2PNetwork {
    pub async fn new(bootnodes: Vec<String>, port: u64, key_path: String) -> Result<Self, anyhow::Error> {
        // let local_key = identity::Keypair::generate_ed25519();
        let local_key = file_tools::load_keypair_from_file(key_path)?;
        let local_peer_id = PeerId::from(local_key.public());
        println!("Local peer id: {:?}", local_peer_id);

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key.clone())
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_quic()
            .with_dns()?
            .with_behaviour(|key| {
                // To content-address message, we can take the hash of message and use it as an ID.
                let message_id_fn = |message: &gossipsub::Message| {
                    let mut s = DefaultHasher::new();
                    message.data.hash(&mut s);
                    gossipsub::MessageId::from(s.finish().to_string())
                };

                // Set a custom gossipsub configuration
                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
                    .validation_mode(gossipsub::ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
                    .message_id_fn(message_id_fn) // content-address messages. No two messages of the same content will be propagated.
                    .build()
                    .map_err(|msg| io::Error::new(io::ErrorKind::Other, msg))?; // Temporary hack because `build` does not return a proper `std::error::Error`.

                // build a gossipsub network behaviour
                let gossipsub = gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                )?;

                let mdns =
                    mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id())?;

                let mut identify_cfg = identify::Config::new(
                    "/ipfs/id/1.0.0".to_string(),
                    key.public(),
                );
                identify_cfg = identify_cfg.with_push_listen_addr_updates(true);
                let identify = identify::Behaviour::new(identify_cfg);

                let mut cfg = kad::Config::default();
                let store = kad::store::MemoryStore::new(key.public().to_peer_id());
                let kademlia = kad::Behaviour::with_config(key.public().to_peer_id(), store, cfg);

                Ok(MyBehaviour { gossipsub, mdns, kademlia, identify })
            })?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
            .build();

        swarm.behaviour_mut().kademlia.set_mode(Some(Mode::Server));



        // Create a Gossipsub topic
        let topic = gossipsub::IdentTopic::new("humanity-net");
        // subscribes to our topic
        swarm.behaviour_mut().gossipsub.subscribe(&topic)?;



        for peer_addr_str in bootnodes {
            if let Ok(multiaddr) = peer_addr_str.parse::<Multiaddr>() {
                if let Some(peer_id) = P2PNetwork::extract_peer_id_from_multiaddr(&multiaddr) {
                    swarm.behaviour_mut().kademlia.add_address(&peer_id, multiaddr);
                } else {
                    println!("PeerId not found in multiaddr");
                }
            } else {
                eprintln!("Failed to parse multiaddr: {}", peer_addr_str);
            }
        }


        // Listen on all interfaces and whatever port the OS assigns
        swarm.listen_on(format!("/ip4/0.0.0.0/udp/{}/quic-v1", port.clone()).parse()?)?;
        swarm.listen_on(format!("/ip4/0.0.0.0/tcp/{}", port).parse()?)?;

        let (command_tx, mut command_rx) = mpsc::channel(64 ); // 创建通道
        let (subscribe_message_tx, _) =  broadcast::channel(1024);;

        let subscribe_message_tx_clone = subscribe_message_tx.clone();
        let mut timer = interval(Duration::from_secs(10));
        let full_addrs = Arc::new(RwLock::new(Vec::new()));
        let full_addrs_clone = full_addrs.clone();

        tokio::spawn(async move {
            loop {
                select! {
                _ = timer.tick() => {
                    // 定时触发的代码
                    if let Err(e) = swarm.behaviour_mut().kademlia.bootstrap() {
                        println!("Bootstrap error: {:?}", e);
                    }
                },
                Some(command) = command_rx.recv() => {
                    match command {
                        SwarmCommand::ConnectPeer(addr, result_tx) => {
                            let dial_result = match swarm.dial(addr) {
                                Ok(_) => Ok(()),
                                Err(e) => {
                                        println!("ConnectPeer err {:?}", e);
                                        Err(anyhow::Error::new(e))
                                    },
                            };
                            // 发送操作结果，忽略发送错误，因为接收端可能已经被丢弃
                            let _ = result_tx.send(dial_result);
                        },
                        SwarmCommand::PublishMessage( message, result_tx) => {
                            let publish_result = swarm.behaviour_mut().gossipsub.publish(topic.clone(), message);
                            // 将成功的Result<MessageId, _>转换为Result<(), _>
                            let result = publish_result.map(|_message_id| ()).map_err(|e| anyhow::Error::new(e));
                            // 发送操作结果
                            let _ = result_tx.send(result);
                        },
                        SwarmCommand::GetConnectedPeers(result_tx) => {
                            let peers = swarm.connected_peers().cloned().collect::<HashSet<_>>();
                            let _ = result_tx.send(Ok(peers)); // 发送结果
                        },
                        SwarmCommand::GetClosestPeers(peer_id) => {
                            swarm.behaviour_mut().kademlia.get_closest_peers(peer_id);
                                // 遍历所有非空的桶
                            for kbucket in swarm.behaviour_mut().kademlia.kbuckets() {
                                println!("Bucket:");

                                // 遍历桶中的每个对等点
                                for entry in kbucket.iter() {
                                    // 打印对等点的 PeerId 和关联的地址
                                    println!("  PeerId: {:?}", entry.node.key.preimage());
                                    println!("  Addresses: {:?}", entry.node.value);
                                }
                            }
                            // 注意：这里我们不能直接发送结果，因为 `get_closest_peers` 是非阻塞的
                            // 而且结果会通过 Kademlia 事件返回
                        },
                        // 处理其他命令
                    }
                }
                event = swarm.select_next_some() => match event {
                    SwarmEvent::Behaviour(MyBehaviourEvent::Identify(identify::Event::Sent { peer_id, .. }) )=> {
                        // println!("Sent identify info to {peer_id:?}")
                    }
                    // Prints out the info received via the identify event
                    SwarmEvent::Behaviour(MyBehaviourEvent::Identify(identify::Event::Received {peer_id,  info })) => {
                        for address in info.listen_addrs.iter() {
                            // 将地址添加到 Kademlia 的路由表中
                            swarm.behaviour_mut().kademlia.add_address(&peer_id, address.clone());
                        }
                        // println!("Received {info:?}")
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                        for (peer_id, multiaddr) in list {
                            println!("mDNS discovered a new peer: {peer_id}");
                            swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                            swarm.behaviour_mut().kademlia.add_address(&peer_id, multiaddr);
                        }
                    },
                    SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                        for (peer_id, _multiaddr) in list {
                            println!("mDNS discover peer has expired: {peer_id}");
                            swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                        }
                    },
                    SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { result,
                            step: ProgressStep { count: _, last },..})) => {
                        match result {
                             kad::QueryResult::GetClosestPeers(Ok(ok)) => {
                                if ok.peers.is_empty() {
                                    println!("Query finished with no closest peers.");
                                    if last {
                                        println!("Failed to find peer on DHT");
                                    }
                                }
                                println!("Query finished with closest peers: {:#?}", ok.peers);
                            }
                            _ => {}
                        }
                    },
                    SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad::Event::RoutingUpdated  {  peer, is_new_peer, addresses, old_peer, ..})) => {
                        // swarm.behaviour_mut().kademlia.bootstrap().expect("bootstrap err");
                        println!("Routing table updated:");
                        println!("  Peer ID: {:?}", peer);
                        println!("  Is new peer: {}", is_new_peer);
                        println!("  Addresses: {:?}", addresses);
                        if let Some(old_peer) = old_peer {
                            println!("  Old peer evicted: {:?}", old_peer);
                        }
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad::Event::UnroutablePeer  {  peer})) => {
                        println!("UnroutablePeer:");
                        println!("  Peer ID: {:?}", peer);
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad::Event::RoutablePeer  {  peer: PeerId, address: Multiaddr})) => {
                        println!("RoutablePeer:");
                        println!("  Peer ID: {:?}", PeerId);
                        println!("  Peer ID: {:?}", Multiaddr);
                    }
                    SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad::Event::PendingRoutablePeer  {  peer: PeerId, address: Multiaddr})) => {
                        println!("PendingRoutablePeer:");
                        println!("  Peer ID: {:?}", PeerId);
                        println!("  Peer ID: {:?}", Multiaddr);
                    }

                    SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                        propagation_source: peer_id,
                        message_id: id,
                        message,
                    })) =>{
                         println!(
                            "Got message with id: {id} from peer: {peer_id}",
                        );

                         let message_event = MessageEvent {
                            source: peer_id,
                            content: message.data.clone(),
                        };
                        // if let Err(e) = subscribe_message_tx.send(message_event).await {
                        //     eprintln!("Error sending message event: {:?}", e);
                        // }
                        let res = subscribe_message_tx_clone.send(message_event);
                        match res {
                            Ok(_) => {}
                            Err(e) => {
                                    println!("send receive MessageEvent err: {:?}", e)
                                }
                        }
                    }
                    SwarmEvent::NewListenAddr { address, .. } => {
                        // println!("Local node is listening on {address}");
                        let full_addr = format!("{}/p2p/{}", address, swarm.local_peer_id());
                        println!("Full address: {}", full_addr);
                        let mut addrs = full_addrs_clone.write().await;
                        addrs.push(full_addr);
                    }
                    SwarmEvent::ConnectionEstablished {
                        peer_id,
                        endpoint,
                        num_established,
                        concurrent_dial_errors: _,
                        established_in: _,
                        connection_id: _,
                    } => {
                            //println!("Got ConnectionEstablished from peer: {peer_id}");
                             // 从 endpoint 中提取地址信息
                            // let addr = match endpoint {
                            //     ConnectedPoint::Dialer { address, .. } => address,
                            //     ConnectedPoint::Listener { send_back_addr, .. } => send_back_addr,
                            // };
                            //
                            // // 尝试将对等节点及其地址添加到路由表
                            // swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                    }
                    SwarmEvent::OutgoingConnectionError {
                        peer_id,
                        connection_id: _,
                        error,
                    } => {
                        match peer_id {
                            None => {}
                            Some(p) => {
                                // println!("Got ConnectionEstablished from peer err: {p}, {error}");
                            }
                        }
                    }


                    _ => {}
                }
            }
            }
        });

        let p2p_network = P2PNetwork {
            // swarm: swarm_arc,
            peer_id: local_peer_id,
            command_tx,
            subscribe_message_tx,
            full_addrs,
            // topic,
        };

        Ok(p2p_network)
    }

    pub async fn connect_to_peer(&self, addr: &str) -> Result<(), anyhow::Error> {
        let addr = addr.parse::<Multiaddr>().context("Error parsing Multiaddr")?;

        let (tx, rx) = oneshot::channel();
        self.command_tx.send(SwarmCommand::ConnectPeer(addr, tx)).await.context("Error sending connection command")?;
        rx.await.context("Error waiting for connection response")?
    }

    pub async fn publish_message(&self, message: Vec<u8>) -> Result<(), anyhow::Error> {
        let (tx, rx) = oneshot::channel();
        self.command_tx.send(SwarmCommand::PublishMessage(message, tx)).await.context("Error sending publish message command")?;
        rx.await.context("Error waiting for message response to be published")?
    }

    // 获取当前直接连接的所有对等节点的 PeerId 列表。
    pub async fn get_connected_peers(&self) -> Result<HashSet<PeerId>, anyhow::Error> {
        let (tx, rx) = oneshot::channel();
        self.command_tx.send(SwarmCommand::GetConnectedPeers(tx)).await.context("Error sending get connected peers command")?;
        rx.await.context("Error waiting for get connected peers response")?
    }

    pub fn subscribe_to_messages(&self) -> broadcast::Receiver<MessageEvent> {
        self.subscribe_message_tx.subscribe()
    }

    pub async fn get_closest_peers(&self, peer_id_str: &str) -> Result<(), anyhow::Error> {
        // 尝试从字符串解析 PeerId
        let peer_id = PeerId::from_str(peer_id_str)
            .context(format!("Error parsing PeerId from string: {}", peer_id_str))?;

        // let (tx, rx) = oneshot::channel();

        self.command_tx.send(SwarmCommand::GetClosestPeers(peer_id)).await
            .context("Error sending get closest peers command")?;
        Ok(())
        // rx.await.context("Error waiting for get closest peers response")?
    }
}


impl P2PNetwork {

    fn extract_peer_id_from_multiaddr(addr: &Multiaddr) -> Option<PeerId> {
        // 遍历Multiaddr中的每个Protocol
        for protocol in addr.iter() {
            if let Protocol::P2p(hash) = protocol {
                // 尝试从p2p部分提取PeerId
                if let Ok(peer_id) = PeerId::from_multihash(hash.into()) {
                    return Some(peer_id);
                }
            }
        }
        None
    }

}