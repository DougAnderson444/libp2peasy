use crate::config;

use super::config::topic::topic;
use libp2p::StreamProtocol;
// TODO: Make a behaviour config module instead of crate wide config
use libp2p::autonat;
use libp2p::gossipsub;
use libp2p::identify;
use libp2p::identity::Keypair;
use libp2p::kad::{record::store::MemoryStore, Kademlia, KademliaConfig, KademliaEvent};
use libp2p::relay;
use libp2p::swarm::{behaviour::toggle::Toggle, keep_alive, NetworkBehaviour};
use libp2p::{Multiaddr, PeerId};
use log::debug;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::time::Duration;
use void::Void;

const IPFS_BOOTNODES: [&str; 4] = [
    "QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
    "QmQCU2EcMqAqQPR2i9bChDtGNJchTbq5TbXJJ16u19uLTa",
    "QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb",
    "QmcZf59bWwK5XFi76CZX8cbJ4BhTzzA3gU1ZjYZcYW3dwt",
];

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "ComposedEvent", prelude = "libp2p::swarm::derive_prelude")]
pub struct Behaviour {
    pub gossipsub: Toggle<gossipsub::Behaviour>,
    identify: identify::Behaviour,
    pub kademlia: Toggle<Kademlia<MemoryStore>>,
    keep_alive: keep_alive::Behaviour,
    relay: relay::Behaviour,
    autonat: Toggle<autonat::Behaviour>,
}

impl Behaviour {
    // This method will help users to discover the builder
    pub fn builder(id_keys: Keypair) -> BehaviourBuilder {
        BehaviourBuilder::new(id_keys)
    }
}

pub struct BehaviourBuilder {
    // Probably lots of optional fields.
    id_keys: Keypair,
    autonat: Option<autonat::Behaviour>,
    kademlia: Option<Kademlia<MemoryStore>>,
    gossipsub: Option<gossipsub::Behaviour>,
}

/// Builder pattern for Behaviour.
/// Defaults to no autonat and no kademlia.
impl BehaviourBuilder {
    // use Builder pattern to build a behaviour
    pub fn new(id_keys: Keypair) -> Self {
        Self {
            id_keys,
            autonat: None,
            kademlia: None,
            gossipsub: None,
        }
    }

    /// Optionally enable autonat
    /// ⚠️ Do not enable for WebRTC-Broswer facing connections, as autonat may break WebRTC Transport in browsers
    pub fn with_autonat(mut self) -> Self {
        self.autonat = Some(autonat::Behaviour::new(
            PeerId::from(self.id_keys.public()),
            Default::default(),
        ));
        self
    }

    /// Optionally active and set the kademlia protocol name
    /// Protocol name example: `/universal-connectivity/lan/kad/1.0.0`
    pub fn with_kademlia(&mut self, protocol_name: Option<&StreamProtocol>) -> &Self {
        // Create a Kademlia behaviour.
        let mut cfg = KademliaConfig::default();
        if let Some(proto) = protocol_name {
            cfg.set_protocol_names(vec![proto.clone()]);
        }
        let store = MemoryStore::new(PeerId::from(self.id_keys.public()));
        let mut kademlia = Kademlia::with_config(PeerId::from(self.id_keys.public()), store, cfg);

        // Maybe use IPFS_BOOTNODES if kad::protocol::DEFAULT_PROTO_NAME is in the iter of protocol_names
        // ie) if default or if KADEMLIA_PROTOCOL_NAME matches kad::protocol::DEFAULT_PROTO_NAME
        if kademlia
            .protocol_names()
            .iter()
            .any(|p| *p == config::IPFS_PROTO_NAME)
        {
            let bootaddr = Multiaddr::from_str("/dnsaddr/bootstrap.libp2p.io").unwrap();
            for peer in &IPFS_BOOTNODES {
                kademlia.add_address(&PeerId::from_str(peer).unwrap(), bootaddr.clone());
            }
            kademlia.bootstrap().unwrap();
        }

        self.kademlia = Some(kademlia);
        self
    }

    /// Optionally enable Gossipsub
    pub fn with_gossipsub(&mut self) -> &Self {
        // To content-address message, we can take the hash of message and use it as an ID.
        let message_id_fn = |message: &gossipsub::Message| {
            let mut s = DefaultHasher::new();
            message.data.hash(&mut s);
            gossipsub::MessageId::from(s.finish().to_string())
        };

        // Set a custom gossipsub configuration
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            // .heartbeat_initial_delay(Duration::from_secs(5))
            // .heartbeat_interval(Duration::from_secs(5)) // This is set to aid debugging by not cluttering the log space
            .validation_mode(gossipsub::ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
            .message_id_fn(message_id_fn) // content-address messages. No two messages of the same content will be propagated.
            // .mesh_outbound_min(1)
            // .mesh_n_low(1)
            .support_floodsub()
            // .flood_publish(true)
            .build()
            .expect("Valid config");

        // build a gossipsub network behaviour
        let mut gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(self.id_keys.clone()),
            gossipsub_config,
        )
        .expect("Correct configuration");

        // subscribes to our topic
        gossipsub.subscribe(&topic()).expect("Valid topic string");

        self.gossipsub = Some(gossipsub);
        self
    }

    /// Builds the [Behaviour]
    pub fn build(self) -> Behaviour {
        let local_peer_id = PeerId::from(&self.id_keys.public());
        debug!("Local peer id: {local_peer_id}");

        // TODO: Tie in identify behaviour with plugin id/version?

        let identify = identify::Behaviour::new(
            identify::Config::new("/ipfs/0.1.0".into(), self.id_keys.public())
                .with_interval(Duration::from_secs(60)) // do this so we can get timeouts for dropped WebRTC connections
                .with_agent_version(format!("rust-libp2p-server/{}", env!("CARGO_PKG_VERSION"))),
        );

        Behaviour {
            identify,
            autonat: self.autonat.into(),
            kademlia: self.kademlia.into(),
            gossipsub: self.gossipsub.into(),
            keep_alive: keep_alive::Behaviour,
            relay: relay::Behaviour::new(
                local_peer_id,
                relay::Config {
                    max_reservations: usize::MAX,
                    max_reservations_per_peer: 100,
                    reservation_rate_limiters: Vec::default(),
                    circuit_src_rate_limiters: Vec::default(),
                    max_circuits: usize::MAX,
                    max_circuits_per_peer: 100,
                    ..Default::default()
                },
            ),
        }
    }
}

#[derive(Debug)]
pub enum ComposedEvent {
    Gossipsub(gossipsub::Event),
    Identify(identify::Event),
    Autonat(autonat::Event),
    Kademlia(KademliaEvent),
    Relay(relay::Event),
}

impl From<gossipsub::Event> for ComposedEvent {
    fn from(event: gossipsub::Event) -> Self {
        ComposedEvent::Gossipsub(event)
    }
}

impl From<identify::Event> for ComposedEvent {
    fn from(event: identify::Event) -> Self {
        ComposedEvent::Identify(event)
    }
}

impl From<autonat::Event> for ComposedEvent {
    fn from(event: autonat::Event) -> Self {
        ComposedEvent::Autonat(event)
    }
}

impl From<KademliaEvent> for ComposedEvent {
    fn from(event: KademliaEvent) -> Self {
        ComposedEvent::Kademlia(event)
    }
}

impl From<relay::Event> for ComposedEvent {
    fn from(event: relay::Event) -> Self {
        ComposedEvent::Relay(event)
    }
}

// void too
impl From<Void> for ComposedEvent {
    fn from(event: Void) -> Self {
        void::unreachable(event)
    }
}
