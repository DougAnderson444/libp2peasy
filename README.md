# Libp2peasy, Lemon Squeezy 🍋✊

Ready made [Libp2p](https://libp2p.io/) [Rust Server](https://github.com/libp2p/rust-libp2p/) that connects to the IPFS DHT and gossipsib.

Design goal is to have all modules separated for easy re-use in other libraries.

## Run Binary

From this package root run:

```bash
cargo run
```

## Hacks & Notes

The server sends a hearbeat message every 15 seconds to the browsers, because Libp2p-WebRTC is still in alpha and gets disconnected after inactivity.

WebRTC is used instead of WebTransport as WebTransport is [not yet supported](https://github.com/libp2p/rust-libp2p/issues/2993) in rust-libp2p.

AutoNat doesn't seem to play well when connected to WebRTC transported browsers, so it is not enabled.
