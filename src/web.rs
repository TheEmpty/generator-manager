use chrono::prelude::Utc;
use rocket::fs::FileServer;
use rocket::{serde::json::Value, State};
use rocket_dyn_templates::{context, Template};
use std::sync::Arc;
use tokio::sync::Mutex;

#[post("/api/v1/do_not_run_generator/<desire>")]
async fn set_do_not_run_generator(state: &State<Arc<Mutex<crate::State>>>, desire: bool) -> Value {
    log::trace!("Taking state lock");
    let mut state = state.lock().await;
    state.config_mut().set_do_not_run_generator(desire);
    let save_result = state.config().save();
    if let Err(ref error) = save_result {
        log::debug!("{}", error);
    }
    log::trace!("droping state lock");
    drop(state);
    rocket::serde::json::json!({
        "success": "true",
        "do_not_run_generator": desire,
        "saved": save_result.is_ok(),
    })
}

#[post("/api/v1/shore_limit/<new_limit>")]
async fn set_shore_limit(state: &State<Arc<Mutex<crate::State>>>, new_limit: f32) -> Value {
    log::trace!("Taking state lock");
    let mut state = state.lock().await;
    state.config_mut().set_shore_limit(new_limit);
    let save_result = state.config().save();
    if let Err(ref error) = save_result {
        log::debug!("{}", error);
    }
    log::trace!("droping state lock");
    drop(state);
    rocket::serde::json::json!({
        "success": "true",
        "shore_limit": new_limit,
        "saved": save_result.is_ok(),
    })
}

#[get("/api/v1/do_not_run_generator")]
async fn get_do_not_run_generator(state: &State<Arc<Mutex<crate::State>>>) -> Value {
    log::trace!("Taking state lock");
    let state = state.lock().await;
    let do_not_run_generator = *state.config().do_not_run_generator();
    log::trace!("droping state lock");
    drop(state);
    rocket::serde::json::json!({
        "do_not_run_generator": do_not_run_generator,
    })
}

#[post("/api/v1/manage/<desire>")]
async fn set_we_turned_it_on(state: &State<Arc<Mutex<crate::State>>>, desire: bool) -> Value {
    log::trace!("Taking state lock");
    let mut state = state.lock().await;
    if desire {
        state.turned_on();
    } else {
        state.turned_off();
    }
    log::trace!("droping state lock");
    drop(state);
    rocket::serde::json::json!({
        "success": true,
    })
}

#[post("/api/v1/timer/<desire>")]
async fn set_timer(state: &State<Arc<Mutex<crate::State>>>, desire: i64) -> Value {
    log::trace!("Taking state lock");
    let mut state = state.lock().await;
    let duration = chrono::Duration::minutes(desire);
    state.set_timer(duration);
    log::trace!("droping state lock");
    drop(state);
    rocket::serde::json::json!({
        "success": true,
    })
}

#[get("/metrics")]
async fn metrics(state: &State<Arc<Mutex<crate::State>>>) -> Template {
    let generator_wanted = crate::generator::generator_wanted();
    log::trace!("Taking state lock");
    let state = state.lock().await;
    let shore_limit = *state.config().shore_limit();
    let we_turned_it_on = *state.we_turned_it_on();
    let timer = state
        .timer()
        .map(|end_time| (end_time - Utc::now()).num_seconds())
        .unwrap_or_default();
    let do_not_run_generator = *state.config().do_not_run_generator();
    log::trace!("droping state lock");
    drop(state);
    Template::render(
        "metrics",
        context! {
            do_not_run_generator: do_not_run_generator,
            generator_wanted: generator_wanted,
            shore_limit: shore_limit,
            we_turned_it_on: we_turned_it_on,
            generator_on_timer_seconds: timer,
        },
    )
}

#[get("/")]
async fn index(state: &State<Arc<Mutex<crate::State>>>) -> Template {
    log::trace!("Taking state lock");
    let state = state.lock().await;
    let do_not_run_generator = *state.config().do_not_run_generator();
    let we_turned_it_on = *state.we_turned_it_on();
    let timer = state
        .timer()
        .map(|end_time| (end_time - Utc::now()).num_minutes());
    let generator_on = matches!(
        state.generator().state().await,
        Ok(lci_gateway::GeneratorState::Running)
    );

    // the floats
    let shore_limit = *state.config().shore_limit();
    let generator_limit = *state.config().generator().limit();
    let low_voltage = *state.config().generator().low_voltage();
    let low_voltage_charge_minutes = *state.config().generator().low_voltage_charge_minutes();
    let auto_start_soc = *state.config().generator().auto_start_soc();
    let stop_charge_soc = *state.config().generator().stop_charge_soc();
    let current_soc = match *state.last_soc() {
        Some(x) => format!("{x:.1}%"),
        None => "Unknown".to_string(),
    };

    log::trace!("droping state lock");
    drop(state);

    // quickly make floats not icky
    // ex: 12.1 should show as 12.1 not 12.100000381469727
    let shore_limit = format!("{shore_limit:.1}");
    let generator_limit = format!("{generator_limit:.1}");
    let low_voltage = format!("{low_voltage:.1}");
    let low_voltage_charge_minutes = format!("{low_voltage_charge_minutes:.1}");
    let auto_start_soc = format!("{auto_start_soc:.1}");
    let stop_charge_soc = format!("{stop_charge_soc:.1}");

    Template::render(
        "index",
        context! {
            do_not_run_generator: do_not_run_generator,
            shore_limit: shore_limit,
            we_turned_it_on: we_turned_it_on,
            generator_on: generator_on,
            timer: timer,
            generator_limit: generator_limit,
            low_voltage: low_voltage,
            low_voltage_charge_minutes: low_voltage_charge_minutes,
            auto_start_soc: auto_start_soc,
            stop_charge_soc: stop_charge_soc,
            current_soc: current_soc,
            build: format!("On build v{} compiled with {} on {}.",
            env!("CARGO_PKG_VERSION"),
            env!("RUSTC_VERSION"),
            env!("DATE_TIME")),
        },
    )
}

pub(crate) async fn launch(state: Arc<Mutex<crate::State>>) {
    let static_path = std::env::var("ROCKET_STATIC_DIR")
        .expect("Please set path to static files via ROCKET_STATIC_DIR");
    let rocket = rocket::build()
        .mount(
            "/",
            routes![
                index,
                set_do_not_run_generator,
                get_do_not_run_generator,
                set_we_turned_it_on,
                set_timer,
                set_shore_limit,
                metrics
            ],
        )
        .mount("/static", FileServer::from(static_path))
        .manage(state)
        .attach(Template::fairing())
        .ignite()
        .await
        .expect("Failed to ignite")
        .launch();
    tokio::spawn(rocket);
}
