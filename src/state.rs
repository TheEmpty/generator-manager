use crate::Config;
use chrono::{
    prelude::{DateTime, Utc},
    Duration,
};
use derive_getters::Getters;
use lci_gateway::{Generator, GeneratorState};

pub(crate) enum DesiredGeneratorState {
    On,
    Off,
    InDesiredState,
}

/// quick solution to the fact that some of the locks() in the right order can cause dead lock.
/// so we'll just take a lock() on State and put anything that needs a Mutex shared between methods here.
#[derive(Getters)]
pub(crate) struct State {
    ac_input: f32,
    generator: Generator,
    config: Config,
    last_soc: Option<f32>,
    last_voltage: Option<f32>,
    last_batt_wattage: Option<f32>,
    we_turned_it_on: bool,
    shore_available: bool,
    timer: Option<DateTime<Utc>>,
}

impl State {
    pub(crate) fn new(generator: Generator, config: Config) -> Self {
        State {
            ac_input: 0f32,
            generator,
            config,
            last_soc: None,
            last_voltage: None,
            last_batt_wattage: None,
            we_turned_it_on: false,
            shore_available: false,
            timer: None,
        }
    }

    pub(crate) fn set_ac_limit(&mut self, input: f32) {
        self.ac_input = input;
    }

    pub(crate) fn set_last_soc(&mut self, input: f32) {
        self.last_soc = Some(input);
    }

    pub(crate) fn set_last_voltage(&mut self, input: f32) {
        self.last_voltage = Some(input);
    }

    pub(crate) fn set_last_batt_wattage(&mut self, input: f32) {
        self.last_batt_wattage = Some(input);
    }

    pub(crate) fn turned_on(&mut self) {
        self.we_turned_it_on = true;
    }

    pub(crate) fn turned_off(&mut self) {
        self.we_turned_it_on = false;
    }

    pub(crate) fn generator_mut(&mut self) -> &mut Generator {
        &mut self.generator
    }

    pub(crate) fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    pub(crate) fn set_shore_availability(&mut self, val: bool) {
        self.shore_available = val;
    }

    pub(crate) fn set_timer(&mut self, duration: Duration) {
        self.timer = Some(Utc::now() + duration);
    }

    pub(crate) async fn wanted(&mut self) -> DesiredGeneratorState {
        if let Some(end_time) = self.timer {
            if end_time <= Utc::now() {
                log::debug!("Removing timer.");
                self.timer = None;
            }
        }

        // Manually turned off
        let gen_on = matches!(self.generator().state().await, Ok(GeneratorState::Running));
        if !gen_on && self.we_turned_it_on {
            log::debug!("Appears to have been manually turned off.");
            self.we_turned_it_on = false;
        }

        let low_battery = match self.last_soc {
            Some(soc) => soc <= *self.config().generator().auto_start_soc(),
            None => false,
        };

        let low_voltage = match self.last_voltage {
            Some(voltage) => voltage <= *self.config().generator().low_voltage(),
            None => false,
        };

        if low_voltage {
            log::warn!("LOW BATTERY VOLTAGE: starting timer");
            self.set_timer(Duration::minutes(
                (*self.config().generator().low_voltage_charge_minutes()).into(),
            ));
        }

        let generator_is_better_than_shore =
            self.config().generator().limit() > self.config().shore_limit();
        let already_charging = match self.last_batt_wattage {
            Some(x) => x > 0f32,
            None => false,
        };

        let charging_conditions = (low_battery || low_voltage || self.timer.is_some())
            && !already_charging
            && !self.config().do_not_run_generator()
            && !gen_on
            && (!self.shore_available || generator_is_better_than_shore);
        if charging_conditions {
            log::debug!(
                "low_battery = {low_battery}, low_voltage = {low_voltage}, timer = {:?}",
                self.timer
            );
            return DesiredGeneratorState::On;
        }

        if gen_on && self.we_turned_it_on && *self.config().do_not_run_generator() {
            log::warn!("Turning off since prevent start was triggered.");
            return DesiredGeneratorState::Off;
        }

        let battery_charged_soc = match self.last_soc {
            Some(soc) => soc >= *self.config().generator().stop_charge_soc(),
            None => false,
        };

        let off_conditions =
            gen_on && self.we_turned_it_on && battery_charged_soc && self.timer.is_none();
        if off_conditions {
            log::debug!("All off conditions met. Going back to the battery!");
            return DesiredGeneratorState::Off;
        }

        DesiredGeneratorState::InDesiredState
    }
}
