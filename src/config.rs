use derive_getters::Getters;
use serde::Deserialize;
use std::{fs::File, io::BufReader};

#[derive(Deserialize, Getters)]
pub(crate) struct MqttCredentials {
    host: String,
    port: u16,
    user: String,
    password: String,
}

#[derive(Deserialize, Getters)]
pub(crate) struct MqttTopics {
    current_limit: String,
    shore_connected: String,
    soc: String,
}

#[derive(Deserialize, Getters)]
pub(crate) struct GeneratorConfig {
    limit: f32,
    auto_start_soc: f32,
    stop_charge_soc: f32,
}

#[derive(Deserialize, Getters)]
pub(crate) struct Config {
    shore_limit: f32,
    mqtt: MqttCredentials,
    topics: MqttTopics,
    generator: GeneratorConfig,
}

impl Config {
    pub(crate) fn load(file: String) -> Self {
        let config_file = File::open(file).expect("Could not find {file}");
        let config_reader = BufReader::new(config_file);
        serde_json::from_reader(config_reader).expect("Error reading configuration.")
    }

    pub(crate) fn load_from_arg() -> Self {
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
        Config::load(config_file)
    }

    pub(crate) fn set_shore_limit(&mut self, new_shore_limit: u8) {
        self.shore_limit = new_shore_limit.into();
    }
}
