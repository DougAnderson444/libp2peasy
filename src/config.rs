use libp2p::identity;
use libp2p::identity::PeerId;
use libp2p::kad::protocol::DEFAULT_PROTO_NAME;
use log::warn;
use serde_derive::{Deserialize, Serialize};
use std::error::Error;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use tokio::fs;

pub const KADEMLIA_PROTOCOL_NAME: &[u8] = b"/universal-connectivity/lan/kad/1.0.0"; // kad::protocol::DEFAULT_PROTO_NAME
pub const LOCAL_KEY_PATH: &str = "./local_keypair";

pub mod topic {
    use libp2p::gossipsub::IdentTopic;

    const IPNS_DEMO: &str = "universal-connectivity";

    pub fn new(topic: String) -> IdentTopic {
        IdentTopic::new(topic)
    }

    pub fn topic() -> IdentTopic {
        IdentTopic::new(IPNS_DEMO)
    }
}

#[derive(Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct Config {
    pub identity: Identity,
}

impl Config {
    pub fn from_file(path: &Path) -> Result<Self, Box<dyn Error>> {
        Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
    }

    pub async fn load_keypair(
        config: &Option<PathBuf>,
    ) -> Result<libp2p::identity::Keypair, Box<dyn Error>> {
        match config {
            Some(path) => {
                println!("Previously saved local peerid available");

                let config = zeroize::Zeroizing::new(Config::from_file(path.as_path())?);

                let keypair = identity::Keypair::from_protobuf_encoding(&zeroize::Zeroizing::new(
                    base64::decode(config.identity.priv_key.as_bytes())?,
                ))?;

                let peer_id = keypair.public().into();
                assert_eq!(
                    PeerId::from_str(&config.identity.peer_id)?,
                    peer_id,
                    "Expect peer id derived from private key and peer id retrieved from config to match."
                );

                Ok(keypair)
            }
            None => {
                warn!("No saved Local peer available");
                let keypair = identity::Keypair::generate_ed25519();

                // Save keypair to file.
                let config = Config {
                    identity: Identity {
                        peer_id: keypair.public().to_peer_id().to_string(),
                        priv_key: base64::encode(
                            keypair.to_protobuf_encoding().expect("valid keypair"),
                        ),
                    },
                };

                match serde_json::to_string_pretty(&config) {
                    Ok(config) => {
                        fs::write(LOCAL_KEY_PATH, config).await?;
                        Ok(keypair)
                    }
                    Err(e) => Err(e.into()),
                }
            }
        }
    }
}

#[derive(Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "PascalCase")]
pub struct Identity {
    #[serde(rename = "PeerID")]
    pub peer_id: String,
    pub priv_key: String,
}

impl zeroize::Zeroize for Config {
    fn zeroize(&mut self) {
        self.identity.peer_id.zeroize();
        self.identity.priv_key.zeroize();
    }
}
