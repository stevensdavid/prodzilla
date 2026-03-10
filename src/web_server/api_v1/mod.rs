mod handlers;
mod model;

use axum::{
    routing::{get, post},
    Router,
};

pub fn router() -> Router {
    Router::new()
        .route(
            "/monitors",
            get(handlers::list_monitors).post(handlers::create_monitor),
        )
        .route("/monitors/summary", get(handlers::monitor_summary))
        .route(
            "/monitors/:name",
            get(handlers::get_monitor)
                .put(handlers::update_monitor)
                .delete(handlers::delete_monitor),
        )
        .route(
            "/monitors/:name/results",
            get(handlers::get_monitor_results),
        )
        .route(
            "/monitors/:name/trigger",
            post(handlers::trigger_monitor),
        )
}
