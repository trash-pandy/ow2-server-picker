use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;

use anyhow::{Result, anyhow};
use surge_ping::{self, Client, Config};
use tokio::runtime::Runtime;
use tokio::sync::broadcast;
use tokio::time;

#[derive(Clone, Debug)]
pub enum PingStatus {
    /// The endpoint has not been pinged yet.
    Unknown,
    /// The endpoint has responded to a ping.
    Reachable(Duration),
    /// The endpoint has not responded to a ping.
    Unreachable,
}

#[derive(Clone, Debug)]
pub struct PingUpdate(pub String, pub PingStatus);

pub type PingReceiver = broadcast::Receiver<PingUpdate>;

pub fn setup_pinger(runtime: &Runtime, endpoints: Vec<(String, IpAddr)>) -> Result<PingReceiver> {
    let (tx, rx) = broadcast::channel(endpoints.len());

    let client = runtime
        .block_on(async move { Client::new(&Config::default()) })
        .map_err(|err| anyhow!("unable to initialize ping client: {}", err))?;

    runtime.spawn(async move {
        let mut pingers = HashMap::new();

        {
            let ident = std::process::id() as u16;

            for (key, host) in endpoints {
                pingers.insert(key, client.pinger(host, ident.into()).await);
            }
        }

        let mut seq = 0u16;

        loop {
            for (key, pinger) in pingers.iter_mut() {
                let tx_clone = tx.clone();
                seq = seq.wrapping_add(1);

                async move {
                    let response = pinger
                        .ping(seq.into(), b"github.com/trash-pandy/ow2-server-picker")
                        .await;

                    let status = match response {
                        Ok((_, duration)) => PingStatus::Reachable(duration),
                        Err(_) => PingStatus::Unreachable,
                    };

                    tx_clone
                        .send(PingUpdate(key.clone(), status))
                        .expect("failed to broadcast a ping update");
                }
                .await;
            }

            time::sleep(Duration::from_secs(10)).await;
        }
    });

    Ok(rx)
}
