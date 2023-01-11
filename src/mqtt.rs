use crate::{config::Config, state::State};
use lci_gateway::GeneratorState;
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
    let shore_connected_read_topic = format!("N/{}", config.topics().shore_connected());

    client
        .subscribe(soc_read_topic, QoS::AtLeastOnce)
        .await
        .expect("Failed to subscribe to SoC state");

    client
        .subscribe(current_limit_read_topic, QoS::AtLeastOnce)
        .await
        .expect("Failed to subscribe to AC CurrentLimit requested state");

    client
        .subscribe(shore_connected_read_topic, QoS::AtLeastOnce)
        .await
        .expect("Failed to subscribe to AC shore connected requested state");

    (client, eventloop)
}

pub(crate) async fn set_ac_limit(
    state: Arc<Mutex<State>>,
    client: Arc<Mutex<AsyncClient>>,
    limit: f32,
) -> Result<(), AcLimitError> {
    log::trace!("Setting ac limit to {limit}");
    let payload = format!("{{\"value\": {:.1}}}", limit);
    log::trace!("Sending ac limit payload = {payload}");
    log::trace!("Locking to get current_limit topic.");
    let state_guard = state.lock().await;
    let topic = format!("W/{}", state_guard.config().topics().current_limit());
    drop(state_guard);
    log::trace!("Dropped state lock. Locking on client.");
    let client_guard = client.lock().await;
    client_guard
        .publish(
            topic,
            QoS::AtLeastOnce,
            false, // retain
            payload,
        )
        .await?;
    drop(client_guard);
    log::trace!("Sent ac limit. Dropped clent lock.");
    Ok(())
}

pub(crate) async fn refresh_topics(
    state: Arc<Mutex<State>>,
    client: Arc<Mutex<AsyncClient>>,
) -> Result<(), rumqttc::ClientError> {
    log::trace!("Waiting for lock on state to get topics");
    let state = state.lock().await;
    let config = state.config();
    let current_limit_topic = format!("R/{}", config.topics().current_limit());
    let soc_topic = format!("R/{}", config.topics().soc());
    let shore_connected_topic = format!("R/{}", config.topics().shore_connected());
    let topics = vec![current_limit_topic, soc_topic, shore_connected_topic];
    log::trace!("Refreshing topics, {:?}- freeing state", topics);
    drop(state);

    log::trace!("Locking client to send refresh topics");
    let client_guard = client.lock().await;
    for topic in topics {
        client_guard
            .publish(
                topic,
                QoS::AtLeastOnce,
                false, // retain
                "please",
            )
            .await?;
    }
    drop(client_guard);

    log::trace!("Client lock dropped. All topics have been refreshed.");
    Ok(())
}

pub(crate) async fn check_current_limit(
    state: Arc<Mutex<State>>,
    client: Arc<Mutex<AsyncClient>>,
) -> Result<(), AcLimitError> {
    log::trace!("Waiting for state lock");
    let state_guard = state.lock().await;
    let config = state_guard.config();
    log::trace!("Received state lock");
    let desired = match state_guard.generator().state().await {
        Ok(GeneratorState::Running) => Some(*config.generator().limit()),
        Ok(GeneratorState::Off) => Some(*config.shore_limit()),
        Ok(GeneratorState::Priming) | Ok(GeneratorState::Starting) | Err(_) => None,
    };

    let ac_input = *state_guard.ac_input();
    log::trace!("Desired AC limit = {:?}, actual = {ac_input}", desired);
    log::trace!("Releasing lock");
    drop(state_guard);

    if let Some(desired) = desired {
        if ac_input != desired {
            log::debug!("Setting ac limit to {desired}, was at {ac_input}");
            set_ac_limit(state, client, desired).await?;
        }
    }
    Ok(())
}

#[derive(Debug, Error)]
pub(crate) enum AcLimitError {
    #[error("Failed to send A/C limit change via mqtt. {0}")]
    RumqttcClientError(rumqttc::ClientError),
}
