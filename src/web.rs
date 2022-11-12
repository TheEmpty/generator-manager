use crate::config::Config;
use rocket::{serde::json::Value, State};
use rocket_dyn_templates::{context, Template};
use std::sync::Arc;
use tokio::sync::Mutex;

#[post("/prevent_start/<desire>")]
async fn set_prevent_start(config: &State<Arc<Mutex<Config>>>, desire: bool) -> Value {
    let mut config = config.lock().await;
    config.set_prevent_start(desire);
    let save_result = config.save();
    if let Err(ref error) = save_result {
        log::debug!("{}", error);
    }
    drop(config);
    rocket::serde::json::json!({
        "success": "true",
        "prevent_start": desire,
        "saved": save_result.is_err(),
    })
}

#[post("/shore_limit/<new_limit>")]
async fn set_shore_limit(config: &State<Arc<Mutex<Config>>>, new_limit: u8) -> Value {
    let mut config = config.lock().await;
    config.set_shore_limit(new_limit);
    let save_result = config.save();
    if let Err(ref error) = save_result {
        log::debug!("{}", error);
    }
    drop(config);
    rocket::serde::json::json!({
        "success": "true",
        "shore_limit": new_limit,
        "saved": save_result.is_err(),
    })
}

#[get("/prevent_start")]
async fn get_prevent_start(config: &State<Arc<Mutex<Config>>>) -> Value {
    let config = config.lock().await;
    let prevent_start = *config.prevent_start();
    drop(config);
    rocket::serde::json::json!({
        "prevent_start": prevent_start,
    })
}

#[get("/metrics")]
async fn metrics(config: &State<Arc<Mutex<Config>>>) -> Template {
    let generator_wanted = crate::generator::generator_wanted();
    let shore_available = crate::generator::shore_available();
    let config = config.lock().await;
    let shore_limit = *config.shore_limit();
    let prevent_start = *config.prevent_start();
    drop(config);
    Template::render(
        "metrics",
        context! {
            prevent_start: prevent_start,
            generator_wanted: generator_wanted,
            shore_limit: shore_limit,
            shore_available: shore_available,
        },
    )
}

#[get("/")]
async fn index(config: &State<Arc<Mutex<Config>>>) -> Template {
    let config = config.lock().await;
    let shore_limit = *config.shore_limit();
    let prevent_start = *config.prevent_start();
    drop(config);
    Template::render(
        "index",
        context! {
            prevent_start: prevent_start,
            shore_limit: shore_limit
        },
    )
}

pub(crate) async fn launch(config: Arc<Mutex<Config>>) {
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
        .manage(config)
        .attach(Template::fairing())
        .ignite()
        .await
        .expect("Failed to ignite")
        .launch();
    tokio::spawn(rocket);
}
