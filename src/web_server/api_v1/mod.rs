mod handlers;
mod model;

use axum::{routing::get, Router};

pub fn router() -> Router {
    Router::new()
        .route(
            "/monitors",
            get(handlers::list_monitors).post(handlers::create_monitor),
        )
        .route(
            "/monitors/:name",
            get(handlers::get_monitor)
                .put(handlers::update_monitor)
                .delete(handlers::delete_monitor),
        )
}
