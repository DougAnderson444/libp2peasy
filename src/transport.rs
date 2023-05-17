use anyhow::{Context, Result};
use futures::future::Either;
use libp2p::core::upgrade;
use libp2p::core::{muxing::StreamMuxerBox, transport::Boxed};
use libp2p::dns;
use libp2p::identity;
use libp2p::noise;
use libp2p::tcp;
use libp2p::yamux;
use libp2p::PeerId;
use libp2p::Transport;
use libp2p_quic as quic;
use libp2p_webrtc as webrtc;
use libp2p_webrtc::tokio::Certificate;
use log::info;
use std::io;
use std::path::Path;
use std::time::Duration;
use tokio::fs;

const LOCAL_CERT_PATH: &str = "./cert.pem";

/// Create a new Transport that supports WebRTC, QUIC, and TCP.
pub async fn create(local_keypair: identity::Keypair) -> Result<Boxed<(PeerId, StreamMuxerBox)>> {
    let authentication_config = {
        let noise_keypair_spec = noise::Keypair::<noise::X25519Spec>::new()
            .into_authentic(&local_keypair)
            .context("Failed to create noise keypair")?;

        noise::NoiseConfig::xx(noise_keypair_spec).into_authenticated()
    };

    let mut yamux_config = yamux::YamuxConfig::default();
    // Enable proper flow-control: window updates are only sent when
    // buffered data has been consumed.
    yamux_config.set_window_update_mode(yamux::WindowUpdateMode::on_read());

    let tcp_transport =
        tcp::tokio::Transport::new(tcp::Config::new().port_reuse(true).nodelay(true))
            .upgrade(upgrade::Version::V1)
            .authenticate(authentication_config)
            .multiplex(yamux_config)
            .timeout(Duration::from_secs(20));

    let quic_transport = {
        let mut config = quic::Config::new(&local_keypair);
        config.support_draft_29 = true;
        quic::tokio::Transport::new(config)
    };

    let webrtc_cert = read_or_create_certificate(Path::new(LOCAL_CERT_PATH))
        .await
        .context("Failed to read certificate")?;

    let webrtc = webrtc::tokio::Transport::new(local_keypair.clone(), webrtc_cert);

    let transport = {
        let dns_quic_or_tcp = dns::TokioDnsConfig::system(
            libp2p::core::transport::OrTransport::new(quic_transport, tcp_transport),
        )?;
        dns_quic_or_tcp.or_transport(webrtc)
    };

    Ok(transport
        .map(|dns_or_webrtc, _| match dns_or_webrtc {
            Either::Left(quic_or_tcp) => match quic_or_tcp {
                Either::Left((peer_id, muxer)) => (peer_id, StreamMuxerBox::new(muxer)),
                Either::Right((peer_id, muxer)) => (peer_id, StreamMuxerBox::new(muxer)),
            },
            Either::Right((peer_id, muxer)) => (peer_id, StreamMuxerBox::new(muxer)),
        })
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
        .boxed())
}

async fn read_or_create_certificate(path: &Path) -> Result<Certificate> {
    if path.exists() {
        let pem = fs::read_to_string(&path).await?;

        info!("Using existing certificate from {}", path.display());

        return Ok(Certificate::from_pem(&pem)?);
    }

    let cert = Certificate::generate(&mut rand::thread_rng())?;
    fs::write(&path, &cert.serialize_pem().as_bytes()).await?;

    info!(
        "Generated new certificate and wrote it to {}",
        path.display()
    );

    Ok(cert)
}
