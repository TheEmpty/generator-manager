use chrono::prelude::Utc;
use rocket::{serde::json::Value, State};
use rocket_dyn_templates::{context, Template};
use std::sync::Arc;
use tokio::sync::Mutex;

#[post("/do_not_run_generator/<desire>")]
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
        "succeGeneratorss": "true",
        "do_not_run_generator": desire,
        "saved": save_result.is_err(),
    })
}

#[post("/shore_limit/<new_limit>")]
async fn set_shore_limit(state: &State<Arc<Mutex<crate::State>>>, new_limit: u8) -> Value {
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
        "saved": save_result.is_err(),
    })
}

#[get("/do_not_run_generator")]
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

#[get("/metrics")]
async fn metrics(state: &State<Arc<Mutex<crate::State>>>) -> Template {
    let generator_wanted = crate::generator::generator_wanted();
    log::trace!("Taking state lock");
    let state = state.lock().await;
    let shore_limit = *state.config().shore_limit();
    let we_turned_it_on = *state.we_turned_it_on();
    let timer = state
        .timer()
        .map(|end_time| (Utc::now() - end_time).num_seconds())
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
    let shore_limit = *state.config().shore_limit();
    let do_not_run_generator = *state.config().do_not_run_generator();
    log::trace!("droping state lock");
    drop(state);
    Template::render(
        "index",
        context! {
            do_not_run_generator: do_not_run_generator,
            shore_limit: shore_limit
        },
    )
}

pub(crate) async fn launch(state: Arc<Mutex<crate::State>>) {
    let rocket = rocket::build()
        .mount(
            "/",
            routes![
                index,
                set_do_not_run_generator,
                get_do_not_run_generator,
                set_shore_limit,
                metrics
            ],
        )
        .manage(state)
        .attach(Template::fairing())
        .ignite()
        .await
        .expect("Failed to ignite")
        .launch();
    tokio::spawn(rocket);
}
