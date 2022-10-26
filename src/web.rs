use rocket::serde::json::Value;
use rocket_dyn_templates::{context, Template};
use std::sync::atomic::{AtomicBool, Ordering};

static PREVENT_START: AtomicBool = AtomicBool::new(false);

#[post("/prevent_start/<desire>")]
fn set_prevent_start(desire: bool) -> Value {
    PREVENT_START.store(desire, Ordering::Relaxed);
    get_prevent_start()
}

#[get("/prevent_start")]
fn get_prevent_start() -> Value {
    let prevent_start = PREVENT_START.load(Ordering::Relaxed);
    rocket::serde::json::json!({
        "prevent_start": prevent_start,
    })
}

#[get("/metrics")]
fn metrics() -> Template {
    let prevent_start = PREVENT_START.load(Ordering::Relaxed);
    Template::render("metrics", context! {prevent_start: prevent_start})
}

#[get("/")]
fn index() -> Template {
    let prevent_start = PREVENT_START.load(Ordering::Relaxed);
    Template::render("index", context! {prevent_start: prevent_start})
}

pub(crate) async fn launch() {
    let rocket = rocket::build()
        .mount(
            "/",
            routes![index, set_prevent_start, get_prevent_start, metrics],
        )
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
