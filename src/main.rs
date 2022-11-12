mod config;
mod error;
mod generator;
mod lci;
mod mqtt;
mod web;

#[macro_use]
extern crate rocket;

use config::Config;
use lci_gateway::{Generator, Tank};
use mqtt::AcLimitError;
use rumqttc::{
    mqttbytes::v4::Packet,
    AsyncClient,
    Event::{Incoming, Outgoing},
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::{sync::Mutex, task::JoinHandle};

#[tokio::main]
async fn main() {
    env_logger::init();
    log::debug!(
        "Built v{} on {} compiled with {}.",
        env!("CARGO_PKG_VERSION"),
        env!("DATE_TIME"),
        env!("RUSTC_VERSION")
    );
    let config = Arc::new(Mutex::new(Config::load_from_arg()));
    web::launch(config.clone()).await;
    let (generator, gas_tank) = lci::get_generator().await;

    // Threading stuff
    let gas_tank = Arc::new(gas_tank);
    let generator = Arc::new(Mutex::new(generator));
    let ac_input = Arc::new(Mutex::new(0f32));
    let control_c = Arc::new(AtomicBool::new(false));
    let control_c_clone = control_c.clone();
    ctrlc::set_handler(move || {
        log::warn!("Control-C recieved. Shutting down.");
        control_c_clone.store(true, Ordering::Relaxed);
    })
    .expect("Error setting Ctrl-C handler");

    loop {
        if control_c.load(Ordering::Relaxed) {
            break;
        }

        let (client, mut eventloop) = {
            let config = config.lock().await;
            mqtt::setup(&config).await
        };
        let client = Arc::new(Mutex::new(client));
        log::debug!("Starting eventloop poll");

        let refresh_task = start_refresh_thread(config.clone(), client.clone());

        while let Ok(notification) = eventloop.poll().await {
            if control_c.load(Ordering::Relaxed) {
                break;
            }

            // create clones here so ownership of a new arc can move instead of original.
            let config = config.clone();
            let gas_tank = gas_tank.clone();
            let client = client.clone();
            let generator = generator.clone();
            let ac_input = ac_input.clone();
            // Spawn in another thread so we don't miss eventloops like ping while waiting which may take minutes.
            tokio::spawn(async move {
                handle_notification(config, client, generator, ac_input, gas_tank, notification)
                    .await;
            });
        }

        refresh_task.abort();
    }
}

async fn handle_notification(
    config: Arc<Mutex<Config>>,
    client: Arc<Mutex<AsyncClient>>,
    generator: Arc<Mutex<Generator>>,
    ac_input: Arc<Mutex<f32>>,
    gas_tank: Arc<Tank>,
    notification: rumqttc::Event,
) {
    log::trace!("Handle notification entry for {:?}", notification);
    match notification {
        Incoming(Packet::Publish(packet)) => {
            let topic = match packet.topic.split('/').last() {
                Some(val) => val.to_lowercase(),
                None => {
                    log::error!("Failed to parse topic {}", packet.topic);
                    return;
                }
            };

            let value: serde_json::Value = match serde_json::from_slice(&packet.payload) {
                Ok(val) => val,
                Err(e) => {
                    log::error!("Failed to convert to JSON {}", e);
                    return;
                }
            };

            log::trace!("Processing: {topic} = {value}");

            if topic == "soc" {
                let value = value["value"].to_string();
                if let Err(error) = generator::check_soc(
                    config.clone(),
                    client.clone(),
                    value,
                    generator.clone(),
                    gas_tank,
                )
                .await
                {
                    log::error!("Error while checking SoC, {}", error);
                }
            } else if topic == "currentlimit" {
                let value = value["value"].to_string();
                if let Ok(val) = value.parse() {
                    log::trace!("Updating ac_input from {val}");
                    *ac_input.lock().await = round_to_half(val);
                    log::trace!("Updated ac_input from {val}");
                } else {
                    log::error!("Failed to parse {value} to f32");
                }
            }

            if let Err(error) = mqtt::check_current_limit(config, client, generator, ac_input).await
            {
                log::error!("Error while checking current limit, {}", error);
            }
        }
        Incoming(Packet::PingReq)
        | Incoming(Packet::PingResp)
        | Outgoing(rumqttc::Outgoing::PingReq)
        | Outgoing(rumqttc::Outgoing::PingResp) => {}
        _ => log::trace!("Unused notification = {:?}", notification),
    };
}

fn start_refresh_thread(
    config: Arc<Mutex<Config>>,
    client: Arc<Mutex<AsyncClient>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        log::trace!("Starting refresh thread.");
        loop {
            log::trace!("refresh: Start");
            if let Err(e) = mqtt::refresh_topics(config.clone(), client.clone()).await {
                log::warn!("Failure refreshing topics. {e}");
            } else {
                log::trace!("Refresh succeeded");
            };
            log::trace!("refresh: Wait");
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            log::trace!("refresh: Loop");
        }
    })
}

fn round_to_half(mut a: f32) -> f32 {
    a *= 10f32; // 10.3 => 103
    a += 4f32; // 103 + 4 = 107
    a = ((a / 5f32) as usize) as f32; // divide, dropping remainder
                                      // 107/5 = 21 with .4 remainder dropped
    a *= 5f32; // rehydrate the chunks: 21 * 5 = 105
    a /= 10f32; // divide by ten to move decimal point, 10.5
    a // Q.E.D.
}
