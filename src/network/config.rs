/// Network Configuration for Ports and Listeners
/// Optional ports for: webrtc, quic, and tcp
/// Optional boolean fags for listeners on webrtc, quic, and tcp
pub struct Config {
    pub webrtc_port: Option<u16>,
    pub quic_port: Option<u16>,
    pub tcp_port: Option<u16>,
    pub listen_webrtc: bool,
    pub listen_quic: bool,
    pub listen_tcp: bool,
}
