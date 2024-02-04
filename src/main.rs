// Copyright 2018 Parity Technologies (UK) Ltd.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use futures::stream::StreamExt;
use libp2p::{gossipsub, mdns, noise, swarm::NetworkBehaviour, swarm::SwarmEvent, tcp, yamux, identity, kad, PeerId, identify};
use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use tokio::{io, io::AsyncBufReadExt, select};
use tracing_subscriber::EnvFilter;
use libp2p::kad::store::MemoryStore;
use libp2p::kad::Mode;
use anyhow::{Result};
use chrono::Utc;
use std::{fs::File, io::{Read, Write}, path::PathBuf};

pub mod p2p;
pub mod file_tools;
// 在 main.rs 或 lib.rs 的其他模块中
use p2p::P2PNetwork;


const BOOTNODES: [&str; 4] = [
    "QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
    "QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa",
    "QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb",
    "QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt",
];

// We create a custom network behaviour that combines Gossipsub and Mdns.
#[derive(NetworkBehaviour)]
struct MyBehaviour {
    gossipsub: gossipsub::Behaviour,
    mdns: mdns::tokio::Behaviour,
    kademlia: kad::Behaviour<MemoryStore>,
}

fn file_test() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let keypair = file_tools::generate_ed25519_keypair();
    // let file_path = PathBuf::from("ed25519_keypair.dat");

    // 将密钥对保存到文件
    file_tools::save_keypair_to_file(&keypair, "./".to_string())?;

    // 从文件加载密钥对
    let loaded_keypair = file_tools::load_keypair_from_file("node.key".to_string())?;

    // 比较生成和读取的公钥是否相同
    assert_eq!(keypair.public(), loaded_keypair.public(), "Public keys do not match");

    // 出于示例目的，这里显示如何安全地处理和打印公钥信息
    println!("Loaded public key: {:?}", loaded_keypair.public());

    Ok(())
}

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn Error>> {
//     demo().await
// }


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // file_test();
    // return Ok(());
    let bootnodes = vec![
        // 这里填入实际的bootnodes地址，例如:
        // "/ip4/104.131.131.82/tcp/4001/p2p/QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb"
        // "/ip4/192.168.101.108/tcp/7366/p2p/12D3KooWB7BojEa87xau1ceb5dV9wwsahrS35BeBmQoitV8DaABk"
        // "/ip4/127.0.0.1/udp/62614/quic-v1/p2p/12D3KooWHsAZWPjFyPVUJhL6RBAqegUWwMdegbSTdKjHvj43STnV".to_string()
    ];

    // 初始化P2P网络
    let p2p_network = P2PNetwork::new(bootnodes, 5000, "D:\\PycharmProject\\PyO3Demo\\p2p_helper\\ed25519_keypair.dat".to_string())
    .await?;

    // 尝试连接到一个对等节点（这里需要一个有效的对等节点地址）
    // let peer_addr = "/ip4/127.0.0.1/tcp/12345/p2p/12D3KooW...";
    // p2p_network.connect_to_peer(peer_addr).await?;

    // 发布一条消息到Gossipsub网络（假设你已经加入了相应的话题）
    // let message = b"Hello, world!".to_vec();
    // p2p_network.publish_message(message).await?;

    // 获取当前直接连接的所有对等节点的PeerId列表
    // let connected_peers = p2p_network.get_connected_peers().await?;
    // println!("Connected peers: {:?}", connected_peers);
    //
    // // 订阅接收到的消息
    let mut receiver = p2p_network.subscribe_to_messages();
    tokio::spawn(async move {
        while let Ok(message_event) = receiver.recv().await {
            println!("Received message from {:?}: {:?}", message_event.source, message_event.content);
            // 处理接收到的消息...
        }
    });
    println!("start...");

// Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines();
    loop {
        select! {
            Ok(Some(line)) = stdin.next_line() => {
                let mut args = line.split(' ');
                match args.next() {
                    Some("Pub") => {
                        let message = b"Hello, world!".to_vec();
                        let timestamp = Utc::now().to_rfc3339(); // 获取当前UTC时间并转换为RFC3339格式的字符串
                        let timestamp_bytes = timestamp.as_bytes();

                        let mut message_with_timestamp = message.clone(); // 克隆原始消息以避免修改
                        message_with_timestamp.extend_from_slice(b" "); // 添加一个空格作为分隔符
                        message_with_timestamp.extend_from_slice(timestamp_bytes); // 添加时间戳

                        p2p_network.publish_message(message_with_timestamp).await?;
                    }
                    Some("Get") => {
                        let connected_peers = p2p_network.get_connected_peers().await?;
                        println!("Connected peers: {:?}", connected_peers);
                    }
                    Some("Connect") => {
                        // let peer_addr = "/ip4/127.0.0.1/tcp/12345/p2p/12D3KooW...";

                        match args.next() {
                            Some(peer_addr) => {
                                let r = p2p_network.connect_to_peer(peer_addr).await?;
                                println!("Connected success");
                            },
                            None => {
                                eprintln!("Expected peer_addr");
                            }
                        }
                    }
                    Some("GetDht") => {
                        // let peer_addr = "/ip4/127.0.0.1/tcp/12345/p2p/12D3KooW...";
                        match args.next() {
                            Some(peer_id) => {
                                let r = p2p_network.get_closest_peers(peer_id).await?;
                                println!("Connected success");
                            },
                            None => {
                                eprintln!("Expected peer_addr");
                            }
                        }
                    }
                    _ => {
                        eprintln!("expected GET, GET_PROVIDERS, PUT or PUT_PROVIDER");
                    }
                }
            }
        }
    }
    // 保持程序运行以监听消息（在实际应用中，你可能需要更复杂的逻辑来管理程序的生命周期）
    tokio::time::sleep(Duration::from_secs(60)).await;
    Ok(())
}



async fn demo() -> Result<(), Box<dyn Error>> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    // Create a random key for ourselves.
    let local_key = identity::Keypair::generate_ed25519();

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


            let mut cfg = kad::Config::default();
            cfg.set_query_timeout(Duration::from_secs(5 * 60));
            let store = kad::store::MemoryStore::new(key.public().to_peer_id());
            let kademlia = kad::Behaviour::with_config(key.public().to_peer_id(), store, cfg);

            Ok(MyBehaviour { gossipsub, mdns, kademlia })
        })?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    swarm.behaviour_mut().kademlia.set_mode(Some(Mode::Server));

    for peer in &BOOTNODES {
        swarm
            .behaviour_mut()
            .kademlia
            .add_address(&peer.parse()?, "/dnsaddr/bootstrap.libp2p.io".parse()?);
    }

    // Create a Gossipsub topic
    let topic = gossipsub::IdentTopic::new("test-net");
    // subscribes to our topic
    swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines();

    // Listen on all interfaces and whatever port the OS assigns
    swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    println!("Enter messages via STDIN and they will be sent to connected peers using Gossipsub");

    // Kick it off
    loop {
        select! {
            Ok(Some(line)) = stdin.next_line() => {
                if let Err(e) = swarm
                    .behaviour_mut().gossipsub
                    .publish(topic.clone(), line.as_bytes()) {
                    println!("Publish error: {e:?}");
                }
            }
            event = swarm.select_next_some() => match event {
                SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                    for (peer_id, _multiaddr) in list {
                        println!("mDNS discovered a new peer: {peer_id}");
                        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                    }
                },
                SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                    for (peer_id, _multiaddr) in list {
                        println!("mDNS discover peer has expired: {peer_id}");
                        swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                    }
                },
                SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(kad::Event::OutboundQueryProgressed { result, ..})) => {
                    match result {
                         kad::QueryResult::GetClosestPeers(Ok(ok)) => {
                             if ok.peers.is_empty() {
                                println!("Query finished with no closest peers.")
                            }
                            println!("Query finished with closest peers: {:#?}", ok.peers);
                        }
                        _ => {}
                    }
                },
                SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                    propagation_source: peer_id,
                    message_id: id,
                    message,
                })) => println!(
                        "Got message: '{}' with id: {id} from peer: {peer_id}",
                        String::from_utf8_lossy(&message.data),
                    ),
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Local node is listening on {address}");
                }
                _ => {}
            }
        }
    }
}
