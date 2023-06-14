use crate::behaviour::{Behaviour, ComposedEvent};
use crate::config::topic;
use libp2p::core::ConnectedPoint;
// use config::Config;
use anyhow::{anyhow, Result};
use libp2p::core::{muxing::StreamMuxerBox, transport::Boxed};
use libp2p::multiaddr::Protocol;
use libp2p::swarm::{AddressRecord, AddressScore, Swarm, SwarmBuilder, SwarmEvent};
use libp2p::{identify, Multiaddr, PeerId};
use log::{debug, error, info, warn};
use std::error::Error;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{self, Receiver};
use tokio::sync::oneshot;
use tokio_stream::StreamExt;

mod types;

const TICK_INTERVAL: Duration = Duration::from_secs(15);

/// Interact with the network:
/// Enables users to spawn a thread on separate networks within the same add_explicit_peer
///
/// - Network Client: Interact with the netowrk by sending
/// - Network Event Loop: Start the network event loop
pub async fn new(
    transport: Boxed<(PeerId, StreamMuxerBox)>,
    behaviour: Behaviour,
    peer_id: PeerId,
) -> Result<(Client, Receiver<NetworkEvent>, EventLoop), Box<dyn Error>> {
    let swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, peer_id).build();

    let (command_sender, command_receiver) = mpsc::channel(8);
    let (event_sender, event_receiver) = mpsc::channel(8);

    Ok((
        Client {
            sender: command_sender,
        },
        event_receiver,
        EventLoop::new(swarm, command_receiver, event_sender),
    ))
}

#[derive(Clone)]
pub struct Client {
    sender: mpsc::Sender<Command>,
}

impl Client {
    /// Listen for incoming connections on the given address.
    pub async fn start_listening(&mut self, addr: Multiaddr) -> Result<(), Box<dyn Error + Send>> {
        let (sender, receiver) = oneshot::channel();
        self.sender
            .send(Command::StartListening { addr, sender })
            .await
            .expect("Command receiver not to be dropped.");
        receiver.await.expect("Sender not to be dropped.")
    }
    pub async fn publish(&mut self, message: String, topic: String) {
        self.sender
            .send(Command::Publish { message, topic })
            .await
            .expect("Command receiver not to be dropped.");
    }
    pub async fn subscribe(&mut self, topic: String) {
        self.sender
            .send(Command::Subscribe { topic })
            .await
            .expect("Command receiver not to be dropped.");
    }
}

#[derive(Debug)]
enum Command {
    StartListening {
        addr: Multiaddr,
        sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
    },
    Dial {
        addr: Multiaddr,
        sender: oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
    },
    Publish {
        message: String,
        topic: String,
    },
    Subscribe {
        topic: String,
    },
}

#[derive(Debug)]
pub enum NetworkEvent {
    NewListenAddr { address: Multiaddr },
    Error { error: anyhow::Error },
}

pub struct EventLoop {
    tick: futures_timer::Delay,
    now: Instant,
    swarm: Swarm<Behaviour>,
    command_receiver: mpsc::Receiver<Command>,
    event_sender: mpsc::Sender<NetworkEvent>,
}

impl EventLoop {
    fn new(
        swarm: Swarm<Behaviour>,
        command_receiver: mpsc::Receiver<Command>,
        event_sender: mpsc::Sender<NetworkEvent>,
    ) -> Self {
        Self {
            tick: futures_timer::Delay::new(TICK_INTERVAL),
            now: Instant::now(),
            swarm,
            command_receiver,
            event_sender,
        }
    }

    pub async fn run(mut self) {
        self.now = Instant::now();

        loop {
            tokio::select! {
                event = self.swarm.next() => self.handle_event(event.expect("Swarm stream to be infinite.")).await,
                command = self.command_receiver.recv() => match command {
                    Some(c) => self.handle_command(c).await,
                    // Command channel closed, thus shutting down the network event loop.
                    None=>  return,
                },
                // also select on whether tick is Ready, then handle_tick
                _ = &mut self.tick => self.handle_tick().await,
            }
        }
    }

    async fn handle_tick(&mut self) {
        eprintln!("🕒 Ticking at {:?}", self.now.elapsed().as_secs());
        self.tick.reset(TICK_INTERVAL);

        debug!(
            "external addrs: {:?}",
            self.swarm
                .external_addresses()
                .collect::<Vec<&AddressRecord>>()
        );

        if let Some(Err(e)) = self
            .swarm
            .behaviour_mut()
            .kademlia
            .as_mut()
            .map(|k| k.bootstrap())
        {
            debug!("Failed to run Kademlia bootstrap: {e:?}");
        }

        let message = format!(
            "Hello world! Sent from the rust-peer at: {:4}s",
            self.now.elapsed().as_secs()
        );

        if let Some(Err(err)) = self
            .swarm
            .behaviour_mut()
            .gossipsub
            .as_mut()
            .map(|g| g.publish(topic::topic(), message.as_bytes()))
        {
            error!("Failed to publish periodic message: {err}")
        }
    }

    async fn handle_event(&mut self, event: SwarmEvent<ComposedEvent, types::ComposedErr>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                let mut addr_handler = || {
                    let p2p_addr = address
                        .clone()
                        .with(Protocol::P2p((*self.swarm.local_peer_id()).into()));

                    info!("Listen p2p address: {p2p_addr:?}");
                    // This address is reachable, add it
                    self.swarm
                        .add_external_address(p2p_addr.clone(), AddressScore::Infinite);

                    // pass the address back to the other task, for display, etc.
                    self.event_sender
                        .send(NetworkEvent::NewListenAddr { address: p2p_addr })
                };
                // Protocol::Ip is the first item in the address vector
                match address.iter().next().unwrap() {
                    Protocol::Ip6(ip6) => {
                        // Only add our globally available IPv6 addresses to the external addresses list.
                        if !ip6.is_loopback() && !ip6.is_unspecified() {
                            addr_handler().await.expect("Receiver not to be dropped.");
                        }
                    }
                    Protocol::Ip4(ip4) => {
                        if !ip4.is_loopback() && !ip4.is_unspecified() {
                            addr_handler().await.expect("Receiver not to be dropped.");
                        }
                    }
                    _ => {}
                }
            }
            SwarmEvent::ConnectionEstablished {
                peer_id,
                endpoint: ConnectedPoint::Listener { send_back_addr, .. },
                established_in,
                ..
            } => {
                eprintln!("✔️  Connection Established to {peer_id} in {established_in:?} on {send_back_addr}");
                info!("Connected to {peer_id}");
            }

            SwarmEvent::OutgoingConnectionError { peer_id, error } => {
                warn!("Failed to dial {peer_id:?}: {error}");

                match (peer_id, &error) {
                    (Some(peer_id), libp2p::swarm::DialError::Transport(details_vector)) => {
                        for (addr, _error) in details_vector.iter() {
                            self.swarm
                                .behaviour_mut()
                                .kademlia
                                .as_mut()
                                .map(|k| k.remove_address(&peer_id, addr));

                            self.swarm.remove_external_address(addr);

                            debug!("Removed ADDR {addr:?} from the routing table (if it was in there).");
                        }
                    }
                    _ => {
                        warn!("{error}");
                    }
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                debug!("Connection to {peer_id} closed: {cause:?}");
            }
            SwarmEvent::Behaviour(ComposedEvent::Relay(e)) => {
                debug!("{:?}", e);
            }
            SwarmEvent::Behaviour(ComposedEvent::Gossipsub(
                libp2p::gossipsub::Event::Message {
                    message_id: _,
                    propagation_source: _,
                    message,
                },
            )) => {
                eprintln!(
                    "📨 Received message from {:?}: {}",
                    message.source,
                    String::from_utf8(message.data).unwrap()
                );

                /* Plugin */
                // iterate through registered Plugins and call on_message
                // for plugin in self.plugins.iter_mut() {
                //     let response = plugin.call("on_message", &message).unwrap();
                //     if let Some(response) = response {
                //         self.event_sender.send(response)
                //     }
                // }
            }
            SwarmEvent::Behaviour(ComposedEvent::Gossipsub(
                libp2p::gossipsub::Event::Subscribed { peer_id, topic },
            )) => {
                debug!("{peer_id} subscribed to {topic}");

                if let Some(g) = self.swarm.behaviour_mut().gossipsub.as_mut() {
                    g.add_explicit_peer(&peer_id)
                };

                // publish a message
                // get the last 4 chars of the peer_id as slice:
                let message = format!(
                    "📨 Welcome subscriber {}! From rust-peer at: {:4}s",
                    &peer_id.to_string()[peer_id.to_string().len() - 4..],
                    self.now.elapsed().as_secs()
                );

                if let Some(Err(err)) = self
                    .swarm
                    .behaviour_mut()
                    .gossipsub
                    .as_mut()
                    .map(|g| g.publish(topic::topic(), message.as_bytes()))
                {
                    error!("Failed to publish periodic message: {err}")
                }
            }
            SwarmEvent::Behaviour(ComposedEvent::Identify(identify::Event::Error {
                peer_id,
                error: libp2p::swarm::ConnectionHandlerUpgrErr::Timeout,
            })) => {
                debug!("Connection to {peer_id} closed due to timeout");

                // When a browser tab closes, we don't get a swarm event
                // maybe there's a way to get this with TransportEvent
                // but for now remove the peer from routing table if there's an Identify timeout

                // Add a counter, kick off after #x tries to connect
                // if the peer is still in the routing table, remove it

                self.swarm
                    .behaviour_mut()
                    .kademlia
                    .as_mut()
                    .map(|k| k.remove_peer(&peer_id));

                if let Some(g) = self.swarm.behaviour_mut().gossipsub.as_mut() {
                    g.remove_explicit_peer(&peer_id)
                };

                self.swarm.disconnect_peer_id(peer_id).unwrap();

                debug!("Removed PEER {peer_id} from the routing table (if it was in there).");
            }
            SwarmEvent::Behaviour(ComposedEvent::Identify(identify::Event::Received {
                peer_id,
                info:
                    identify::Info {
                        listen_addrs,
                        protocols,
                        observed_addr,
                        ..
                    },
            })) => {
                debug!("identify::Event::Received observed_addr: {}", observed_addr);

                self.swarm
                    .add_external_address(observed_addr, AddressScore::Infinite);

                // TODO: This needs to be improved to only add the address tot he matching protocol name , assuming there is more than one per kad (which there shouldn't be, but there could be)
                if protocols.iter().any(|p| {
                    self.swarm
                        .behaviour()
                        .kademlia
                        .as_ref()
                        .unwrap()
                        .protocol_names()
                        .iter()
                        .any(|q| p.as_bytes() == q.as_ref())
                }) {
                    for addr in listen_addrs {
                        debug!("identify::Event::Received listen addr: {}", addr);

                        let webrtc_address = addr
                            .clone()
                            .with(Protocol::WebRTCDirect)
                            .with(Protocol::P2p(peer_id.into()));

                        self.swarm
                            .behaviour_mut()
                            .kademlia
                            .as_mut()
                            .map(|k| k.add_address(&peer_id, webrtc_address.clone()));

                        // TODO (fixme): the below doesn't work because the address is still missing /webrtc/p2p even after https://github.com/libp2p/js-libp2p-webrtc/pull/121
                        self.swarm
                            .behaviour_mut()
                            .kademlia
                            .as_mut()
                            .map(|k| k.add_address(&peer_id, addr.clone()));

                        debug!("Added {webrtc_address} to the routing table.");
                    }
                }
            }
            SwarmEvent::Behaviour(ComposedEvent::Kademlia(
                libp2p::kad::KademliaEvent::OutboundQueryProgressed {
                    result: libp2p::kad::QueryResult::Bootstrap(res),
                    ..
                },
            )) => {
                debug!("Kademlia Bootstrap Result: {:?}", res);
            }
            SwarmEvent::Behaviour(ComposedEvent::Kademlia(event)) => {
                debug!("Kademlia event: {:?}", event)
            }
            event => debug!("Other type of event: {:?}", event),
        }
    }

    async fn handle_command(&mut self, command: Command) {
        match command {
            Command::StartListening { addr, sender } => {
                let _ = match self.swarm.listen_on(addr) {
                    Ok(_) => sender.send(Ok(())),
                    Err(e) => sender.send(Err(Box::new(e))),
                };
            }
            Command::Dial { addr, sender } => {
                let _ = match self.swarm.dial(addr) {
                    Ok(_) => sender.send(Ok(())),
                    Err(e) => sender.send(Err(Box::new(e))),
                };
            }
            Command::Publish { message, topic } => {
                if let Some(Err(err)) = self
                    .swarm
                    .behaviour_mut()
                    .gossipsub
                    .as_mut()
                    .map(|g| g.publish(topic::new(topic), message.as_bytes()))
                {
                    error!("Failed to publish message: {err}");
                    let _ = self
                        .event_sender
                        .send(NetworkEvent::Error {
                            error: anyhow!("Trouble sending message: {}", err),
                        })
                        .await;
                }
            }
            Command::Subscribe { topic } => {
                if let Some(Err(err)) = self
                    .swarm
                    .behaviour_mut()
                    .gossipsub
                    .as_mut()
                    .map(|g| g.subscribe(&topic::new(topic)))
                {
                    error!("Failed to subscribe to topic: {err}");
                    let _ = self
                        .event_sender
                        .send(NetworkEvent::Error {
                            error: anyhow!("Trouble subscribing to topic: {}", err),
                        })
                        .await;
                }
            }
        }
    }
}
