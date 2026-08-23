//! Protocol dump. Default path does **not** call a model.
//!
//! ```text
//! cargo run --example dump_gateway
//! cargo run --example dump_gateway -- --prompt "Say hi in one sentence."
//! ```
//!
//! `--prompt` waits for `session.info` then submits. That hits your configured
//! Hermes provider (network, possible cost, up to the RPC timeout).

use std::time::Duration;

use clap::Parser;
use serde_json::json;
use talaria::discover;
use talaria::gateway::{GatewayClient, GatewayEvent};
use talaria::logging::{self, log_line};
use tokio::time::timeout;

#[derive(Parser, Debug)]
#[command(
    about = "Dump live tui_gateway JSON-RPC. Default: ready → session.create → session.close (no model)."
)]
struct Args {
    /// Optional prompt. Calls your configured provider. Waits for session.info first.
    #[arg(long)]
    prompt: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logging::init_file_logging();
    let args = Args::parse();

    if let Some(text) = &args.prompt {
        eprintln!("NOTE: --prompt will call your configured Hermes model with: {text:?}");
    }

    let found = match discover::discover() {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{}", talaria::user_messages::discovery_failed(&e));
            std::process::exit(2);
        }
    };
    eprintln!("python: {}", found.python.display());
    if let Some(root) = &found.src_root {
        eprintln!("src_root: {}", root.display());
    }

    let mut client = GatewayClient::spawn(found.spawn_args()).await?;
    eprintln!("gateway.ready");

    let mut events = client.take_events().expect("events");

    let created = client
        .request("session.create", json!({ "cwd": ".", "cols": 80 }))
        .await?;
    println!("session.create result:");
    println!("{}", serde_json::to_string_pretty(&created)?);
    log_line(&format!("dump session.create {created}"));

    let session_id = created
        .get("session_id")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    eprintln!("session_id={session_id}");
    if let Some(stored) = created.get("stored_session_id") {
        eprintln!("stored_session_id={stored}");
    }

    if let Some(text) = args.prompt {
        eprintln!("waiting for session.info before submit…");
        let info = wait_session_info(&mut events, Duration::from_secs(15)).await;
        match info {
            Some(ev) => eprintln!("session.info: {ev:?}"),
            None => eprintln!("no session.info within 15s; submitting anyway"),
        }
        let submit = client
            .request(
                "prompt.submit",
                json!({ "session_id": session_id, "text": text }),
            )
            .await?;
        println!("prompt.submit result (not end of turn):");
        println!("{}", serde_json::to_string_pretty(&submit)?);

        eprintln!("streaming events until message.complete (Ctrl+C to abort)…");
        loop {
            match timeout(Duration::from_secs(120), events.recv()).await {
                Ok(Some(GatewayEvent::Event(ev))) => {
                    println!("event {} {}", ev.type_name, ev.payload);
                    if ev.type_name == "message.complete" {
                        break;
                    }
                }
                Ok(Some(other)) => println!("gw {other:?}"),
                Ok(None) => break,
                Err(_) => {
                    eprintln!("timed out waiting for message.complete");
                    break;
                }
            }
        }
    }

    if !session_id.is_empty() {
        let _ = client
            .request("session.close", json!({ "session_id": session_id }))
            .await;
    }
    client.shutdown().await?;
    eprintln!("shutdown ok");
    Ok(())
}

async fn wait_session_info(
    events: &mut tokio::sync::mpsc::Receiver<GatewayEvent>,
    max: Duration,
) -> Option<talaria::protocol::WireEvent> {
    let deadline = tokio::time::Instant::now() + max;
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        if left.is_zero() {
            return None;
        }
        match timeout(left, events.recv()).await {
            Ok(Some(GatewayEvent::Event(ev))) if ev.type_name == "session.info" => {
                return Some(ev);
            }
            Ok(Some(_)) => continue,
            Ok(None) => return None,
            Err(_) => return None,
        }
    }
}
