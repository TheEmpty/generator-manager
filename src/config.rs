use derive_getters::Getters;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::BufReader};
use thiserror::Error;

#[derive(Serialize, Deserialize, Getters)]
pub(crate) struct MqttCredentials {
    host: String,
    port: u16,
    user: String,
    password: String,
}

#[derive(Serialize, Deserialize, Getters)]
pub(crate) struct MqttTopics {
    current_limit: String,
    shore_connected: String,
    soc: String,
}

#[derive(Serialize, Deserialize, Getters)]
pub(crate) struct GeneratorConfig {
    limit: f32,
    auto_start_soc: f32,
    stop_charge_soc: f32,
}

#[derive(Serialize, Deserialize, Getters)]
pub(crate) struct Config {
    shore_limit: f32,
    mqtt: MqttCredentials,
    topics: MqttTopics,
    generator: GeneratorConfig,
    #[serde(default)]
    prevent_start: bool,
}

fn get_config_path() -> String {
    match std::env::args().nth(1) {
        Some(x) => {
            log::debug!("Using argument for config file: '{x}'.");
            x
        }
        None => {
            log::debug!("Using default config file path, ./config.json");
            "config.json".to_string()
        }
    }
}

impl Config {
    pub(crate) fn load_from_arg() -> Self {
        let file_path = get_config_path();
        let config_file = File::open(file_path).expect("Could not find {file_path}");
        let config_reader = BufReader::new(config_file);
        serde_json::from_reader(config_reader).expect("Error reading configuration.")
    }

    pub(crate) fn set_shore_limit(&mut self, new_shore_limit: u8) {
        self.shore_limit = new_shore_limit.into();
    }

    pub(crate) fn set_prevent_start(&mut self, prevent_start: bool) {
        self.prevent_start = prevent_start;
    }

    pub(crate) fn save(&self) -> Result<(), SaveConfigError> {
        let json = serde_json::to_string_pretty(&self).map_err(SaveConfigError::Json)?;
        std::fs::write(get_config_path(), json).map_err(SaveConfigError::Io)?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub(crate) enum SaveConfigError {
    #[error("Failed to save config. {0}")]
    Io(std::io::Error),
    #[error("Failed to convert config to JSON. {0}")]
    Json(serde_json::Error),
}
