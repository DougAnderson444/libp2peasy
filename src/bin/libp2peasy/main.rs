use anyhow::Result;
use libp2peasy::Message;
use libp2peasy::Server;
use libp2peasy::ServerResponse;
use tokio::sync::{mpsc, oneshot};

#[tokio::main]
async fn main() -> Result<()> {
    let (sendr, recvr) = mpsc::channel::<Message<ServerResponse>>(8);

    let _handle = tokio::spawn(async {
        let _ = Server::new()
            .enable_kademlia()
            .start_with_tokio_executor(recvr)
            .await;
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
