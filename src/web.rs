use crate::config::Config;
use rocket::{serde::json::Value, State};
use rocket_dyn_templates::{context, Template};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::Mutex;

static PREVENT_START: AtomicBool = AtomicBool::new(false);

#[post("/prevent_start/<desire>")]
fn set_prevent_start(desire: bool) -> Value {
    PREVENT_START.store(desire, Ordering::Relaxed);
    get_prevent_start()
}

#[post("/shore_limit/<new_limit>")]
async fn set_shore_limit(config: &State<Arc<Mutex<Config>>>, new_limit: u8) -> Value {
    let mut config = config.lock().await;
    config.set_shore_limit(new_limit);
    drop(config);
    rocket::serde::json::json!({
        "success": true,
    })
}

#[get("/prevent_start")]
fn get_prevent_start() -> Value {
    let prevent_start = PREVENT_START.load(Ordering::Relaxed);
    rocket::serde::json::json!({
        "prevent_start": prevent_start,
    })
}

#[get("/metrics")]
async fn metrics(config: &State<Arc<Mutex<Config>>>) -> Template {
    let prevent_start = PREVENT_START.load(Ordering::Relaxed);
    let generator_wanted = crate::generator::generator_wanted();
    let shore_available = crate::generator::shore_available();
    let config = config.lock().await;
    let shore_limit = *config.shore_limit();
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
    let prevent_start = PREVENT_START.load(Ordering::Relaxed);
    let config = config.lock().await;
    let shore_limit = *config.shore_limit();
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

pub(crate) fn prevent_start() -> bool {
    PREVENT_START.load(Ordering::Relaxed)
}
