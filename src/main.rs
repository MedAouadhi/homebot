mod bot_commands;
mod utils;
use anyhow::Result;
use bot_commands::commands::MyCommands;
use polybot::server::BotServer;
use polybot::telegram::bot::TelegramBot;
use polybot::Bot;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::select;
use tokio::sync::Notify;
use tokio::time::Duration;
use tracing::{debug, error, info};
type MyBot<'a> = TelegramBot<MyCommands>;
const IP_CHECK_TIME: Duration = Duration::from_secs(60);

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Configure tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let conf = utils::get_config().await?;
    let bot = Arc::new(MyBot::new(conf.clone().bot));

    let bot_clone = bot.clone();
    let conf_clone = conf.clone();
    let config_changed = Arc::new(Notify::new());
    let config_changed_clone = config_changed.clone();

    tokio::spawn(async move {
        loop {
            // explicity handle the result as we are in async block
            if let Ok(current_ip) = utils::get_ip().await {
                debug!("Current ip = {:?}", current_ip);
                if !bot_clone.is_webhook_configured(&current_ip).await.unwrap() {
                    info!("Certificate is not correclty configured, configuring ...");
                } else {
                    // the webhook is already set
                    tokio::time::sleep(IP_CHECK_TIME).await;
                    continue;
                }

                // generate new certificate
                if BotServer::<MyBot>::generate_certificate(
                    PathBuf::from(&conf.server.pubkey_path),
                    PathBuf::from(&conf.server.privkey_path),
                    &current_ip,
                )
                .await
                .is_ok()
                {
                    if bot_clone
                        .update_webhook_cert(PathBuf::from(&conf.server.pubkey_path), &current_ip)
                        .await
                        .is_err()
                    {
                        error!("failed to upload the certificate!");
                    } else {
                        // notify the server that a new certificate has been uploaded
                        config_changed_clone.notify_one();
                    }
                } else {
                    error!("The certificate generation failed!");
                }
            }
            tokio::time::sleep(IP_CHECK_TIME).await;
        }
    });

    loop {
        let mut server = BotServer::new(conf_clone.server.clone(), bot.clone());

        // the flow will block here, until one of the branches terminates, which is due to:
        // - The server terminates by itself (e.g crash ..)
        // - The system's IP has changed
        select! {
            _ = server.start() => {break;},
            // A server restart needs to happen as the certificate has been changed.
            _ = config_changed.notified() => {
                debug!("Received certificate update notification, restarting server ...");
                server.stop().await;
                continue;
            }
        }
    }

    Ok(())
}
