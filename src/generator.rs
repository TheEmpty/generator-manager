use crate::state::{DesiredGeneratorState, State};
use crate::{mqtt, mqtt::AcLimitError};
use lci_gateway::{GeneratorState, Tank};
use rumqttc::AsyncClient;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use thiserror::Error;
use tokio::sync::Mutex;

// these should probably be in state
static GENERATOR_WANTED: AtomicBool = AtomicBool::new(false);

pub(crate) fn generator_wanted() -> bool {
    GENERATOR_WANTED.load(Ordering::Relaxed)
}

pub(crate) async fn check_shore_available(
    state: Arc<Mutex<State>>,
    value: String,
) -> Result<(), CheckShoreError> {
    let available: f32 = value.parse().map_err(CheckShoreError::ParseFloat)?;
    let mut state_guard = state.lock().await;
    state_guard.set_shore_availability(available != 0.0);
    Ok(())
}

pub(crate) async fn update_soc(
    state: Arc<Mutex<State>>,
    client: Arc<Mutex<AsyncClient>>,
    value: String,
    gas_tank: Arc<Tank>,
) -> Result<(), CheckSocError> {
    log::trace!("Trying to lock on state");
    let mut state_guard = state.lock().await;
    log::trace!("Locked state");

    log::trace!("Reading SoC update");
    let soc: f32 = value.parse()?;
    state_guard.set_last_soc(soc);
    drop(state_guard);
    log::trace!("Released config lock");

    check_generator_state(state, client, gas_tank).await;
    Ok(())
}

pub(crate) async fn update_voltage(
    state: Arc<Mutex<State>>,
    client: Arc<Mutex<AsyncClient>>,
    value: String,
    gas_tank: Arc<Tank>,
) -> Result<(), CheckSocError> {
    log::trace!("Trying to lock on state");
    let mut state_guard = state.lock().await;
    log::trace!("Locked state");

    log::trace!("Reading voltage update");
    let soc: f32 = value.parse()?;
    state_guard.set_last_voltage(soc);
    drop(state_guard);
    log::trace!("Released config lock");

    check_generator_state(state, client, gas_tank).await;
    Ok(())
}

pub(crate) async fn update_batt_wattage(
    state: Arc<Mutex<State>>,
    client: Arc<Mutex<AsyncClient>>,
    value: String,
    gas_tank: Arc<Tank>,
) -> Result<(), CheckSocError> {
    log::trace!("Trying to lock on state");
    let mut state_guard = state.lock().await;
    log::trace!("Locked state");

    log::trace!("Reading batt wattage update");
    let soc: f32 = value.parse()?;
    state_guard.set_last_batt_wattage(soc);
    drop(state_guard);
    log::trace!("Released config lock");

    check_generator_state(state, client, gas_tank).await;
    Ok(())
}

async fn check_generator_state(
    state: Arc<Mutex<State>>,
    client: Arc<Mutex<AsyncClient>>,
    gas_tank: Arc<Tank>,
) {
    log::trace!("Trying to lock on state");
    let mut state_guard = state.lock().await;
    log::trace!("Locked state");
    let wanted = state_guard.wanted().await;
    drop(state_guard);

    match wanted {
        DesiredGeneratorState::On => {
            let _ = turn_on(state, client, gas_tank).await;
        }
        DesiredGeneratorState::Off => {
            let _ = turn_off(state, client).await;
        }
        DesiredGeneratorState::InDesiredState => {}
    }
}

async fn turn_off(
    state: Arc<Mutex<State>>,
    client: Arc<Mutex<AsyncClient>>,
) -> Result<(), GeneratorOffError> {
    log::info!("Want to turn generator off");
    let state_guard = state.clone();
    let mut state_guard = state_guard.lock().await;
    log::trace!("Have state lock");
    state_guard.turned_off();
    let gen_state = state_guard.generator().state().await?;
    if gen_state != GeneratorState::Running {
        return Err(GeneratorOffError::InvalidState(gen_state));
    }

    let desired = *state_guard.config().shore_limit();
    log::trace!("Dropping state guard to call set_ac_limit");
    drop(state_guard);
    mqtt::set_ac_limit(state.clone(), client, desired).await?;
    log::trace!("Getting lock again to send generator off");
    let mut state_guard = state.lock().await;
    log::info!("Sending generator off");
    state_guard.generator_mut().off().await?;

    // 180 = max time we want to wait, 3 minutes or 180 seconds.
    for _ in 0..180 {
        let state = state_guard.generator().state().await;
        if matches!(state, Ok(GeneratorState::Off)) {
            log::info!("Generator is off");
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    Ok(())
}

async fn turn_on(
    state: Arc<Mutex<State>>,
    client: Arc<Mutex<AsyncClient>>,
    gas_tank: Arc<Tank>,
) -> Result<(), GeneratorOnError> {
    log::info!("Generator is wanted. Checking conditions.");

    if let Ok(perecentage) = gas_tank.level().await {
        if perecentage.value() == 0 {
            return Err(GeneratorOnError::NoGas);
        }
    }

    let mut state_guard = state.lock().await;
    log::trace!("Have state lock");
    state_guard.turned_on();
    let gen = state_guard.generator_mut();
    let gen_state = gen.state().await?;
    if gen_state != GeneratorState::Off {
        return Err(GeneratorOnError::InvalidState(gen_state));
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
    let gen_state = gen.state().await?;
    if gen_state != GeneratorState::Running {
        return Err(GeneratorOnError::FailedToTurnOn(gen_state));
    }
    // TODO: config option for how long to wait
    let desired = *state_guard.config().generator().limit();
    drop(state_guard);
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;

    mqtt::set_ac_limit(state, client, desired).await?;
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
