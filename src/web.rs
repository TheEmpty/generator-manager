use rocket::{serde::json::Value, State};
use rocket_dyn_templates::{context, Template};
use std::sync::Arc;
use tokio::sync::Mutex;

#[post("/prevent_start/<desire>")]
async fn set_prevent_start(state: &State<Arc<Mutex<crate::State>>>, desire: bool) -> Value {
    log::trace!("Taking state lock");
    let mut state = state.lock().await;
    state.config_mut().set_prevent_start(desire);
    let save_result = state.config().save();
    if let Err(ref error) = save_result {
        log::debug!("{}", error);
    }
    log::trace!("droping state lock");
    drop(state);
    rocket::serde::json::json!({
        "succeGeneratorss": "true",
        "prevent_start": desire,
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

#[get("/prevent_start")]
async fn get_prevent_start(state: &State<Arc<Mutex<crate::State>>>) -> Value {
    log::trace!("Taking state lock");
    let state = state.lock().await;
    let prevent_start = *state.config().prevent_start();
    log::trace!("droping state lock");
    drop(state);
    rocket::serde::json::json!({
        "prevent_start": prevent_start,
    })
}

#[get("/metrics")]
async fn metrics(state: &State<Arc<Mutex<crate::State>>>) -> Template {
    let generator_wanted = crate::generator::generator_wanted();
    log::trace!("Taking state lock");
    let state = state.lock().await;
    let shore_limit = *state.config().shore_limit();
    let prevent_start = *state.config().prevent_start();
    log::trace!("droping state lock");
    drop(state);
    Template::render(
        "metrics",
        context! {
            prevent_start: prevent_start,
            generator_wanted: generator_wanted,
            shore_limit: shore_limit,
        },
    )
}

#[get("/")]
async fn index(state: &State<Arc<Mutex<crate::State>>>) -> Template {
    log::trace!("Taking state lock");
    let state = state.lock().await;
    let shore_limit = *state.config().shore_limit();
    let prevent_start = *state.config().prevent_start();
    log::trace!("droping state lock");
    drop(state);
    Template::render(
        "index",
        context! {
            prevent_start: prevent_start,
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
                set_prevent_start,
                get_prevent_start,
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
