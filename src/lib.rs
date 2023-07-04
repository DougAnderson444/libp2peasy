// only if cfg target not wasm32-unknown-unknown
#![cfg(not(target_arch = "wasm32"))]

use crate::config::LOCAL_KEY_PATH;
pub use crate::config::{IPFS_PROTO_NAME, KADEMLIA_PROTOCOL_NAME};

use anyhow::Result;
use bytes::Bytes;
use libp2p::multiaddr::{Multiaddr, Protocol};
use libp2p::StreamProtocol;
use log::warn;
use std::error::Error;
use std::net::Ipv6Addr;
use std::path::Path;
use std::path::PathBuf;
use tokio::sync::{mpsc, oneshot};

pub mod behaviour;
pub mod config;
pub mod network;
pub mod transport;

const PORT_WEBRTC: u16 = 9090;
const PORT_QUIC: u16 = 9091;
const PORT_TCP: u16 = 9092;

type Responder<T> = oneshot::Sender<T>;

#[derive(Debug, Clone)]
pub struct ServerResponse {
    pub address: Bytes,
}

pub struct Message<T> {
    pub reply: Responder<T>,
}

#[derive(Debug, Default)]
pub struct Libp2peasy {
    /// Path to IPFS config file.
    config: Option<PathBuf>,

    /// Whether to run the libp2p Kademlia protocol and join the IPFS DHT.
    enable_kademlia: bool,

    /// Name of the Kademlia protocol.
    kademlia_name: Option<StreamProtocol>,

    /// Whether to run the libp2p Autonat protocol.
    enable_autonat: bool,

    /// Whether to run the libp2p Gossipsub protocol.
    enable_gossipsub: bool,

    /// Address to listen on
    listen_address: Option<String>,

    /// Address of a remote peer to connect to
    remote_address: Option<Multiaddr>,
}

impl Libp2peasy {
    pub fn new() -> Self {
        Libp2peasy {
            config: None,
            enable_kademlia: false,
            kademlia_name: None,
            enable_autonat: false,
            enable_gossipsub: false,
            listen_address: None,
            remote_address: None,
        }
    }

    fn with_config(&mut self, config: PathBuf) -> &mut Libp2peasy {
        self.config = Some(config);
        self
    }

    pub fn enable_gossipsub(&mut self) -> &mut Libp2peasy {
        self.enable_gossipsub = true;
        self
    }

    pub fn enable_kademlia(&mut self, name: StreamProtocol) -> &mut Libp2peasy {
        self.kademlia_name = Some(name);
        self.enable_kademlia = true;
        self
    }

    pub fn enable_autonat(&mut self) -> &mut Libp2peasy {
        self.enable_autonat = true;
        self
    }

    // with listen address
    pub fn with_listen_address(&mut self, listen_address: String) -> &mut Libp2peasy {
        self.listen_address = Some(listen_address);
        self
    }

    // with remote address
    pub fn with_remote_address(&mut self, remote_address: Multiaddr) -> &mut Libp2peasy {
        self.remote_address = Some(remote_address);
        self
    }

    /// TODO: Optional Plugins
    pub fn with_plugin(&mut self) -> &mut Libp2peasy {
        // TODO: new HashMap<String, Plugin> in self to track the plugins by name? or just a vec of plugins?
        self
    }

    /// An example WebRTC peer that will accept connections
    pub async fn start_with_tokio_executor(
        &mut self,
        mut request_recvr: mpsc::Receiver<Message<ServerResponse>>,
    ) -> Result<(), Box<dyn Error>> {
        if self.config.is_none() {
            // check if local key exists
            match config::Config::from_file(Path::new(LOCAL_KEY_PATH)) {
                Ok(_) => {
                    let _ = &self.with_config(Path::new(LOCAL_KEY_PATH).into());
                }
                Err(_) => {
                    warn!("No saved Local peer available");
                }
            }
        }

        let local_keypair = config::Config::load_keypair(&self.config).await?;

        let transport = transport::create(local_keypair.clone()).await?;

        let mut behaviour_builder = behaviour::BehaviourBuilder::new(local_keypair.clone());

        if let Some(name) = &self.kademlia_name {
            behaviour_builder.with_kademlia(Some(name));
        }

        if self.enable_gossipsub {
            behaviour_builder.with_gossipsub();
        };

        let behaviour = behaviour_builder.build();

        // Create networks with behaviours, transports, and PeerId
        // Each network is isolated by the Kad::protocol_name in the behaviour
        // TODO: Each network operator can manage the pubsub topics too

        let (mut network_client, mut network_events, network_event_loop) =
            network::new(transport, behaviour, local_keypair.public().into()).await?;

        // Spawn the network task for it to run in the background.
        let network_handle = tokio::spawn(async move { network_event_loop.run().await });

        // Handle any network events
        tokio::spawn(async move {
            loop {
                match network_events.recv().await {
                    Some(network::NetworkEvent::NewListenAddr { address }) => {
                        // padd message up to main
                        if let Some(message) = request_recvr.recv().await {
                            let _ = message.reply.send(ServerResponse {
                                address: Bytes::from(address.to_string()),
                            });
                        }
                    }
                    evt => {
                        eprintln!("Network event: {:?}", evt);
                    }
                }
            }
        });

        let address_webrtc = Multiaddr::from(Ipv6Addr::UNSPECIFIED)
            .with(Protocol::Udp(PORT_WEBRTC))
            .with(Protocol::WebRTCDirect);

        let address_quic = Multiaddr::from(Ipv6Addr::UNSPECIFIED)
            .with(Protocol::Udp(PORT_QUIC))
            .with(Protocol::QuicV1);

        let address_tcp = Multiaddr::from(Ipv6Addr::UNSPECIFIED).with(Protocol::Tcp(PORT_TCP));

        for addr in [address_webrtc, address_quic, address_tcp] {
            network_client
                .start_listening(addr)
                .await
                .expect("Listening not to fail.");
        }

        network_handle.await?;
        println!("EOF");

        Ok(())
    }
}
