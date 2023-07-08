use anyhow::Result;
use clap::Parser;
use libp2p::Multiaddr;
use libp2peasy::Libp2peasy;
use libp2peasy::Message;
use libp2peasy::ServerResponse;
#[allow(unused_imports)]
use libp2peasy::{IPFS_PROTO_NAME, KADEMLIA_UNIVERSAL_CONNECTIVITY};
use tokio::sync::{mpsc, oneshot};

#[derive(Debug, Parser)]
#[clap(name = "universal connectivity rust peer")]
struct Opt {
    /// Address of a remote peer to connect to.
    #[clap(long)]
    remote: Option<Multiaddr>,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let opt = Opt::parse();

    let (sendr, recvr) = mpsc::channel::<Message<ServerResponse>>(8);

    let _handle = tokio::spawn(async {
        let mut binding = Libp2peasy::new();
        binding
            .enable_kademlia(KADEMLIA_UNIVERSAL_CONNECTIVITY) // could choose custom, or libp2peasy::IPFS_PROTO_NAME (ipfs)
            .enable_gossipsub();

        // if opt.remote, then set up temp config directory for our second set of keys
        if opt.remote.is_some() {
            // Random temp dir for the config file (keys)
            let rand_temp_dir =
                std::env::temp_dir().join(format!("temp-{}", rand::random::<u32>()));
            binding
                .with_remote_address(opt.remote)
                .with_config(rand_temp_dir);
        }
        // todo: .with_plugin(&plugin)
        let _result = binding.start_with_tokio_executor(recvr).await;
    });

    tokio::spawn(async move {
        loop {
            let (reply_sender, reply_rcvr) = oneshot::channel();

            let _result = sendr
                .send(Message::<ServerResponse> {
                    reply: reply_sender,
                })
                .await;

            if let Ok(reply) = reply_rcvr.await {
                let s: String = std::str::from_utf8(&reply.address).unwrap().into();
                // Rust doesn't support octal character escape sequence
                // For colors, use hexadecimal escape instead, plus a series of semicolon-separated parameters.
                println!("Connect with: \n\x1b[30;1;42m{s}\x1b[0m");
            }
        }
    });

    println!("\n*** To Shutdown, use Ctrl + C ***\n");

    match tokio::signal::ctrl_c().await {
        Ok(()) => {}
        Err(err) => {
            eprintln!("Unable to listen for shutdown signal: {err}");
            // we also shut down in case of error
        }
    };

    Ok(())
}
