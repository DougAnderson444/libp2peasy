# Libp2peasy, Lemon Squeezy 🍋✊

Note: [WIP]

Ready made [Libp2p](https://libp2p.io/) [Rust Server](https://github.com/libp2p/rust-libp2p/) that connects to the IPFS DHT and gossipsib.

Design goal is to have all modules separated for easy re-use in other libraries, and yto use a plugin system to extend the functionality of the server as you need it.

## Run Binary

From this package root run:

```bash
cargo run
```

## Build Your Own

```rs
let _join_handle = tokio::spawn(async {
        let _res = Libp2peasy::new()
            .enable_kademlia(/* todo: name */)
            .enable_gossipsub()
            // todo: with_plugin(&plugin)
            .start_with_tokio_executor(recvr)
            .await;
    });
```

## Tests

```bash
cargo test --workspace
```

## Plugins

Libp2peasy doesn't do much on its own except connect with peers and propagate messages. But, it is designed to be extended with _plugins_. Each plugin will have an emitter and handler built into it to transmit and receive messages throught libp2peasy.

More to come as dev continues...

## Hacks & Notes

The server sends a hearbeat message every 15 seconds to the browsers, because Libp2p-WebRTC is still in alpha and gets disconnected after inactivity.

WebRTC is used instead of WebTransport as WebTransport is [not yet supported](https://github.com/libp2p/rust-libp2p/issues/2993) in rust-libp2p.

AutoNat doesn't seem to play well when connected to WebRTC transported browsers, so it is not enabled.
