use crate::config::Config;
use lci_gateway::{Generator, GeneratorState};
use rumqttc::{AsyncClient, EventLoop, MqttOptions, QoS};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Mutex;

pub(crate) async fn setup(config: &Config) -> (AsyncClient, EventLoop) {
    let mut mqttoptions = MqttOptions::new(
        config.mqtt().user().clone(),
        config.mqtt().host().clone(),
        *config.mqtt().port(),
    );
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    mqttoptions.set_credentials(
        config.mqtt().user().clone(),
        config.mqtt().password().clone(),
    );
    let (client, eventloop) = AsyncClient::new(mqttoptions, 10);
    let soc_read_topic = format!("N/{}", config.topics().soc());
    let current_limit_read_topic = format!("N/{}", config.topics().current_limit());

    client
        .subscribe(soc_read_topic, QoS::AtLeastOnce)
        .await
        .expect("Failed to subscribe to SoC state");

    client
        .subscribe(current_limit_read_topic, QoS::AtLeastOnce)
        .await
        .expect("Failed to subscribe to AC CurrentLimit requested state");

    (client, eventloop)
}

pub(crate) async fn set_ac_limit(
    config: Arc<Mutex<Config>>,
    client: Arc<Mutex<AsyncClient>>,
    limit: f32,
) -> Result<(), AcLimitError> {
    log::trace!("Setting ac limit to {limit}");
    let payload = format!("{{\"value\": {:.1}}}", limit);
    log::trace!("Sending ac limit payload = {payload}");
    let topic = format!("W/{}", config.lock().await.topics().current_limit());
    client
        .lock()
        .await
        .publish(
            topic,
            QoS::AtLeastOnce,
            false, // retain
            payload,
        )
        .await?;
    log::trace!("Sent ac limit.");
    Ok(())
}

pub(crate) async fn check_current_limit(
    config: Arc<Mutex<Config>>,
    client: Arc<Mutex<AsyncClient>>,
    gen: Arc<Mutex<Generator>>,
    ac_input: Arc<Mutex<f32>>,
) -> Result<(), AcLimitError> {
    log::trace!(
        "Waiting for lock on generator and ac_input to ensure state does not change during setting limit."
    );
    let gen = gen.lock().await;
    let ac_input = ac_input.lock().await;
    log::trace!("Received generator and ac_input lock in check_current_limit");
    let config_guard = config.clone();
    let config_guard = config_guard.lock().await;
    let desired = match gen.state().await {
        Ok(GeneratorState::Running) => Some(config_guard.generator().limit()),
        Ok(GeneratorState::Off) => Some(config_guard.shore_limit()),
        Ok(GeneratorState::Priming) | Ok(GeneratorState::Starting) | Err(_) => None,
    };

    log::trace!("Desired AC limit = {:?}, actual = {ac_input}", desired);

    if let Some(desired) = desired {
        if *ac_input != *desired {
            log::debug!("Setting ac limit to {desired}, was at {ac_input}");
            set_ac_limit(config, client, *desired).await?;
        }
    }
    Ok(())
}

#[derive(Debug, Error)]
pub(crate) enum AcLimitError {
    #[error("Failed to send A/C limit change via mqtt. {0}")]
    RumqttcClientError(rumqttc::ClientError),
}
