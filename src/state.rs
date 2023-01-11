use crate::Config;
use derive_getters::Getters;
use lci_gateway::Generator;

/// quick solution to the fact that some of the locks() in the right order can cause dead lock.
/// so we'll just take a lock() on State and put anything that needs a Mutex shared between methods here.
#[derive(Getters)]
pub(crate) struct State {
    ac_input: f32,
    generator: Generator,
    config: Config,
}

impl State {
    pub(crate) fn new(generator: Generator, config: Config) -> Self {
        State {
            ac_input: 0f32,
            generator,
            config,
        }
    }

    pub(crate) fn set_ac_limit(&mut self, input: f32) {
        self.ac_input = input;
    }

    pub(crate) fn generator_mut(&mut self) -> &mut Generator {
        &mut self.generator
    }

    pub(crate) fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }
}
