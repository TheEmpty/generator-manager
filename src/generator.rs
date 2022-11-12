use crate::{config::Config, mqtt, mqtt::AcLimitError};
use lci_gateway::{Generator, GeneratorState, Tank};
use rumqttc::AsyncClient;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use thiserror::Error;
use tokio::sync::Mutex;

static GENERATOR_WANTED: AtomicBool = AtomicBool::new(false);
static SHORE_AVAILABLE: AtomicBool = AtomicBool::new(false);

pub(crate) fn generator_wanted() -> bool {
    GENERATOR_WANTED.load(Ordering::Relaxed)
}

pub(crate) fn shore_available() -> bool {
    SHORE_AVAILABLE.load(Ordering::Relaxed)
}

pub(crate) fn check_shore_available(value: String) -> Result<(), CheckShoreError> {
    let available: f32 = value.parse().map_err(CheckShoreError::ParseFloat)?;
    SHORE_AVAILABLE.store(available != 0.0, Ordering::Relaxed);
    Ok(())
}

pub(crate) async fn check_soc(
    config: Arc<Mutex<Config>>,
    client: Arc<Mutex<AsyncClient>>,
    value: String,
    gen: Arc<Mutex<Generator>>,
    gas_tank: Arc<Tank>,
) -> Result<(), CheckSocError> {
    log::trace!("Locking on config");
    let config_guard = config.clone();
    let config_guard = config_guard.lock().await;

    let generator_is_better_than_shore =
        *config_guard.generator().limit() > *config_guard.shore_limit();
    let shore_available = shore_available();
    if shore_available && !generator_is_better_than_shore {
        log::trace!("short circuting since generator limit is lower than shore limit");
        return Ok(());
    }

    log::trace!("Reading SoC update");
    let soc: f32 = value.parse()?;
    let low_bat = soc <= *config_guard.generator().auto_start_soc();
    let high_bat = soc >= *config_guard.generator().stop_charge_soc();
    let prevent_start = *config_guard.prevent_start();
    drop(config_guard);

    log::trace!("Released config lock");
    let gen_on = matches!(gen.lock().await.state().await, Ok(GeneratorState::Running));
    log::trace!("soc = {soc}, low_bat = {low_bat}, high_bat = {high_bat}, gen_on = {gen_on}, prevent_start = {prevent_start}");

    if gen_on && high_bat {
        GENERATOR_WANTED.store(false, Ordering::Relaxed);
        turn_off(config, client, gen).await?;
    } else if !gen_on && low_bat && !prevent_start {
        GENERATOR_WANTED.store(true, Ordering::Relaxed);
        if !prevent_start {
            turn_on(config, client, gen, gas_tank).await?;
        }
    }
    Ok(())
}

async fn turn_off(
    config: Arc<Mutex<Config>>,
    client: Arc<Mutex<AsyncClient>>,
    gen: Arc<Mutex<Generator>>,
) -> Result<(), GeneratorOffError> {
    log::info!("Want to turn generator off");
    let mut gen = gen.lock().await;
    log::trace!("Locked generator");
    let state = gen.state().await?;
    if state != GeneratorState::Running {
        return Err(GeneratorOffError::InvalidState(state));
    }

    mqtt::set_ac_limit(config, client, 15f32).await?;
    log::info!("Sending generator off");
    gen.off().await?;
    for _ in 0..180 {
        let state = gen.state().await;
        if matches!(state, Ok(GeneratorState::Off)) {
            log::info!("Generator is off");
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    Ok(())
}

async fn turn_on(
    config: Arc<Mutex<Config>>,
    client: Arc<Mutex<AsyncClient>>,
    gen: Arc<Mutex<Generator>>,
    gas_tank: Arc<Tank>,
) -> Result<(), GeneratorOnError> {
    log::info!("Generator is wanted. Checking conditions.");

    if let Ok(perecentage) = gas_tank.level().await {
        if perecentage.value() == 0 {
            return Err(GeneratorOnError::NoGas);
        }
    }

    log::trace!("Attempting to take lock for generator as we have fuel.");
    let mut gen = gen.lock().await;
    log::trace!("Lock recieved for turning generator on.");
    let state = gen.state().await?;
    if state != GeneratorState::Off {
        return Err(GeneratorOnError::InvalidState(state));
    }

    log::info!("Sending generator command to start.");
    gen.on().await?;

    // 180 = max time we want to wait, 3 minutes or 180 seconds.
    for _ in 0..180 {
        let state = gen.state().await;
        if matches!(state, Ok(GeneratorState::Running)) {
            log::info!("Generator started");
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    let state = gen.state().await?;
    if state != GeneratorState::Running {
        return Err(GeneratorOnError::FailedToTurnOn(state));
    }
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    mqtt::set_ac_limit(config, client, 45.5).await?;
    Ok(())
}

#[derive(Debug, Error)]
pub(crate) enum GeneratorOnError {
    #[error("Tried to turn on the generator when it was not off, but was {0}")]
    InvalidState(GeneratorState),
    #[error("The generator did not start in time after sending the command to the LCI gateway. Expected it to be running, but it was {0}")]
    FailedToTurnOn(GeneratorState),
    #[error("No gas available to start generator.")]
    NoGas,
    #[error("Failed to send to LCI gateway. {0}")]
    SetError(lci_gateway::SetError),
    #[error("Failed to set AC limit. {0}")]
    AcLimitError(AcLimitError),
    #[error("Trouble understanding the LCI gateway. {0}")]
    GeneratorStateConversionError(lci_gateway::GeneratorStateConversionError),
}

#[derive(Debug, Error)]
pub(crate) enum GeneratorOffError {
    #[error("Failed to set AC limit. {0}")]
    SetAcLimit(AcLimitError),
    #[error("Tried to turn off the generator when it was not running, but was {0}")]
    InvalidState(GeneratorState),
    #[error("Failed to send to LCI gateway. {0}")]
    SetError(lci_gateway::SetError),
    #[error("Trouble understanding the LCI gateway. {0}")]
    GeneratorStateConversionError(lci_gateway::GeneratorStateConversionError),
}

#[derive(Debug, Error)]
pub(crate) enum CheckSocError {
    #[error("Failed to turn generator on. {0}")]
    GeneratorOn(GeneratorOnError),
    #[error("Failed to turn generator off. {0}")]
    GeneratorOff(GeneratorOffError),
    #[error("Failed to parse battery charge. {0}")]
    ParseFloat(std::num::ParseFloatError),
}

#[derive(Debug, Error)]
pub(crate) enum CheckShoreError {
    #[error("Failed to parse 'connected' value. {0}")]
    ParseFloat(std::num::ParseFloatError),
}
