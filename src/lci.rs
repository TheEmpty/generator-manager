use lci_gateway::{DeviceType, Generator, Tank};

pub(crate) async fn get_generator() -> (Generator, Tank) {
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

    (generator, gas_tank)
}
