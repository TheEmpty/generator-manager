use lci_gateway::{DeviceType, Generator, GeneratorState, Tank};
use rumqttc::mqttbytes::v4::Packet;
use rumqttc::Event::Incoming;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::Deserialize;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    env_logger::init();

    // Load config
    let config_file = match std::env::args().nth(1) {
        Some(x) => {
            log::debug!("Using argument for config file: '{x}'.");
            x
        }
        None => {
            log::debug!("Using default config file path, ./config.json");
            "config.json".to_string()
        }
    };
    let config = Arc::new(Config::load(config_file));

    let mut mqttoptions = MqttOptions::new(
        config.mqtt.user.clone(),
        config.mqtt.host.clone(),
        config.mqtt.port,
    );
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    mqttoptions.set_credentials(config.mqtt.user.clone(), config.mqtt.password.clone());
    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    let soc_read_topic = format!("N/{}", config.topics.soc);
    let current_limit_read_topic = format!("N/{}", config.topics.current_limit);

    client
        .subscribe(soc_read_topic, QoS::AtLeastOnce)
        .await
        .expect("Failed to subscribe to SoC state");

    client
        .subscribe(current_limit_read_topic, QoS::AtLeastOnce)
        .await
        .expect("Failed to subscribe to AC CurrentLimit requested state");

    let mut things = lci_gateway::get_things()
        .await
        .expect("Couldn't get lci things");

    let generator_index = things
        .iter()
        .position(|thing| thing.get_type() == Some(DeviceType::Generator));
    let generator_thing = things.remove(generator_index.expect("Failed to find generator"));
    let generator = Generator::new(generator_thing).expect("Failed to create generator");

    let generator_gas_index = things
        .iter()
        .position(|thing| thing.label() == "Generator Fuel Tank");
    let generator_gas_thing = things.remove(generator_gas_index.expect("Failed to find fuel tank"));
    let gas_tank = Tank::new(generator_gas_thing).expect("Failed to create fuel tank");

    // Threading stuff
    let gas_tank = Arc::new(gas_tank);
    let client = Arc::new(Mutex::new(client));
    let generator = Arc::new(Mutex::new(generator));
    let ac_input = Arc::new(Mutex::new(0f32));
    let low_gas_tank_notification = Arc::new(Mutex::new(false));

    log::debug!("Starting eventloop poll");
    while let Ok(notification) = eventloop.poll().await {
        log::trace!("Notification incoming = {:?}", notification);
        // create clones here so ownership of a new arc can move instead of original.
        let config = config.clone();
        let gas_tank = gas_tank.clone();
        let client = client.clone();
        let generator = generator.clone();
        let ac_input = ac_input.clone();
        let low_gas_tank_notification = low_gas_tank_notification.clone();
        // Spawn in another thread so we don't miss eventloops like ping while waiting which may take minutes.
        tokio::spawn(async move {
            handle_notification(
                config,
                client,
                generator,
                low_gas_tank_notification,
                ac_input,
                gas_tank,
                notification,
            )
            .await;
        });
    }
}

async fn handle_notification(
    config: Arc<Config>,
    client: Arc<Mutex<AsyncClient>>,
    generator: Arc<Mutex<Generator>>,
    low_gas_tank_notification: Arc<Mutex<bool>>,
    ac_input: Arc<Mutex<f32>>,
    gas_tank: Arc<Tank>,
    notification: rumqttc::Event,
) {
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
                    log::error!("Failed to convert to JSON {:?}", e);
                    return;
                }
            };

            log::trace!("Processing: {topic} = {value}");

            if topic == "soc" {
                let value = value["value"].to_string();
                let res = check_soc(
                    config.clone(),
                    client.clone(),
                    value,
                    generator.clone(),
                    gas_tank,
                    low_gas_tank_notification,
                )
                .await;
                if let Err(error) = res {
                    log::error!("Error while checking SoC, {:?}", error);
                }
            } else if topic == "currentlimit" {
                let value = value["value"].to_string();
                if let Ok(val) = value.parse() {
                    log::trace!("Updated ac_input from {val}");
                    *ac_input.lock().await = round_to_half(val);
                } else {
                    log::error!("Failed to parse {value} to f32");
                }
            }

            if let Err(error) = check_current_limit(config, client, generator, ac_input).await {
                log::error!("Error while checking current limit, {:?}", error);
            }
        }
        _ => log::trace!("Unused notification = {:?}", notification),
    };
}

async fn check_current_limit(
    config: Arc<Config>,
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
    let desired = match gen.state().await {
        Ok(GeneratorState::Running) => Some(config.generator.limit),
        Ok(GeneratorState::Off) => Some(config.shore_limit),
        Ok(GeneratorState::Priming) | Ok(GeneratorState::Starting) | Err(_) => None,
    };
    log::trace!("Desired AC limit = {:?}, actual = {ac_input}", desired);

    if let Some(desired) = desired {
        if *ac_input != desired {
            log::debug!("Setting ac limit to {desired}, was at {ac_input}");
            set_ac_limit(config, client, desired).await?;
        }
    }
    Ok(())
}

async fn check_soc(
    config: Arc<Config>,
    client: Arc<Mutex<AsyncClient>>,
    value: String,
    gen: Arc<Mutex<Generator>>,
    gas_tank: Arc<Tank>,
    low_gas_tank_notification: Arc<Mutex<bool>>,
) -> Result<(), CheckSocError> {
    log::trace!("Handling SoC update");
    let soc: f32 = value.parse()?;
    let low_bat = soc <= config.generator.auto_start_soc;
    let high_bat = soc >= config.generator.stop_charge_soc;
    let gen_on = matches!(gen.lock().await.state().await, Ok(GeneratorState::Running));
    log::trace!("soc = {soc}, low_bat = {low_bat}, high_bat = {high_bat}, gen_on = {gen_on}");

    if gen_on && high_bat {
        generator_off(config, client, gen).await?;
    } else if !gen_on && low_bat {
        generator_on(config, client, gen, gas_tank, low_gas_tank_notification).await?;
    }
    Ok(())
}

async fn generator_off(
    config: Arc<Config>,
    client: Arc<Mutex<AsyncClient>>,
    gen: Arc<Mutex<Generator>>,
) -> Result<(), GeneratorOffError> {
    log::info!("Want to turn generator off");
    let mut gen = gen.lock().await;
    log::trace!("Locked generator");
    let state = gen.state().await;
    if !matches!(state, Ok(GeneratorState::Running)) {
        log::warn!("Can't turn off generator since it's not in the running state. It was in the {:?} state.", state);
        return Err(GeneratorOffError::InvalidState);
    }

    set_ac_limit(config, client, 15f32).await?;
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

async fn generator_on(
    config: Arc<Config>,
    client: Arc<Mutex<AsyncClient>>,
    gen: Arc<Mutex<Generator>>,
    gas_tank: Arc<Tank>,
    low_gas_tank_notification: Arc<Mutex<bool>>,
) -> Result<(), GeneratorOnError> {
    log::info!("Generator is wanted. Checking conditions.");

    if let Ok(perecentage) = gas_tank.level().await {
        if perecentage.value() == 0 {
            log::warn!("Not enough gas to run generator even though it's wanted.");

            log::debug!("Taking lock on low_gas_tank_notification");
            let mut low_gas_tank_notification = low_gas_tank_notification.lock().await;
            log::debug!("Lock on low_gas_tank_notification taken");
            if !*low_gas_tank_notification {
                *low_gas_tank_notification = true;
                let notification = prowl::Notification::new(
                    config.prowl_api_keys.clone(),
                    Some(prowl::Priority::Emergency),
                    None, // link
                    "Generator".to_string(),
                    "No Fuel".to_string(),
                    "Not enough gas to run generator even though it's wanted.".to_string(),
                )?;
                notification.add().await?;
            }
        }
    }

    log::trace!("Attempting to take lock for generator as we have fuel.");
    let mut gen = gen.lock().await;
    log::trace!("Lock recieved for turning generator on.");
    let state = gen.state().await;
    if !matches!(state, Ok(GeneratorState::Off)) {
        log::warn!(
            "Can't turn on generator since it's not in the off state. It was in the {:?} state.",
            state
        );
        return Err(GeneratorOnError::BadState);
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
    let state = gen.state().await;
    if !matches!(state, Ok(GeneratorState::Running)) {
        log::error!(
            "Failed to turn on generator. Expected running, but was {:?}",
            state
        );
        return Err(GeneratorOnError::FailedToTurnOn);
    }
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    set_ac_limit(config, client, 45.5).await?;
    Ok(())
}

async fn set_ac_limit(
    config: Arc<Config>,
    client: Arc<Mutex<AsyncClient>>,
    limit: f32,
) -> Result<(), AcLimitError> {
    log::trace!("Setting ac limit to {limit}");
    let payload = format!("{{\"value\": {:.1}}}", limit);
    log::trace!("Sending ac limit payload = {payload}");
    let topic = format!("W/{}", config.topics.current_limit);
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

fn round_to_half(mut a: f32) -> f32 {
    a *= 10f32; // 10.3 => 103
    a += 4f32; // 103 + 4 = 107
    a = ((a / 5f32) as usize) as f32; // divide, dropping remainder
                                      // 107/5 = 21 with .4 remainder dropped
    a *= 5f32; // rehydrate the chunks: 21 * 5 = 105
    a /= 10f32; // divide by ten to move decimal point, 10.5
    a // Q.E.D.
}

#[derive(Deserialize)]
struct MqttCredentials {
    host: String,
    port: u16,
    user: String,
    password: String,
}

#[derive(Deserialize)]
struct MqttTopics {
    current_limit: String,
    soc: String,
}

#[derive(Deserialize)]
struct GeneratorConfig {
    limit: f32,
    auto_start_soc: f32,
    stop_charge_soc: f32,
}

#[derive(Deserialize)]
struct Config {
    shore_limit: f32,
    prowl_api_keys: Vec<String>,
    mqtt: MqttCredentials,
    topics: MqttTopics,
    generator: GeneratorConfig,
}

impl Config {
    pub fn load(file: String) -> Self {
        let config_file = File::open(file).expect("Could not find {file}");
        let config_reader = BufReader::new(config_file);
        serde_json::from_reader(config_reader).expect("Error reading configuration.")
    }
}

#[derive(Debug)]
enum CheckSocError {
    GeneratorOn(GeneratorOnError),
    GeneratorOff(GeneratorOffError),
    ParseFloat(std::num::ParseFloatError),
}

#[derive(Debug)]
enum GeneratorOffError {
    SetAcLimit(AcLimitError),
    InvalidState,
    SetError(lci_gateway::SetError),
}

#[derive(Debug)]
enum GeneratorOnError {
    Creation(prowl::CreationError),
    AddError(prowl::AddError),
    BadState,
    FailedToTurnOn,
    SetError(lci_gateway::SetError),
    AcLimitError(AcLimitError),
}

#[derive(Debug)]
enum AcLimitError {
    RumqttcClientError(rumqttc::ClientError),
}

impl From<rumqttc::ClientError> for AcLimitError {
    fn from(error: rumqttc::ClientError) -> Self {
        Self::RumqttcClientError(error)
    }
}

impl From<AcLimitError> for GeneratorOffError {
    fn from(error: AcLimitError) -> Self {
        Self::SetAcLimit(error)
    }
}

impl From<lci_gateway::SetError> for GeneratorOffError {
    fn from(error: lci_gateway::SetError) -> Self {
        Self::SetError(error)
    }
}

impl From<prowl::CreationError> for GeneratorOnError {
    fn from(error: prowl::CreationError) -> Self {
        Self::Creation(error)
    }
}

impl From<prowl::AddError> for GeneratorOnError {
    fn from(error: prowl::AddError) -> Self {
        Self::AddError(error)
    }
}

impl From<lci_gateway::SetError> for GeneratorOnError {
    fn from(error: lci_gateway::SetError) -> Self {
        Self::SetError(error)
    }
}

impl From<AcLimitError> for GeneratorOnError {
    fn from(error: AcLimitError) -> Self {
        Self::AcLimitError(error)
    }
}

impl From<GeneratorOffError> for CheckSocError {
    fn from(error: GeneratorOffError) -> Self {
        Self::GeneratorOff(error)
    }
}

impl From<GeneratorOnError> for CheckSocError {
    fn from(error: GeneratorOnError) -> Self {
        Self::GeneratorOn(error)
    }
}

impl From<std::num::ParseFloatError> for CheckSocError {
    fn from(error: std::num::ParseFloatError) -> Self {
        Self::ParseFloat(error)
    }
}
