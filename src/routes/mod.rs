use axum::{
    handler::HandlerWithoutStateExt,
    http::StatusCode,
    routing::{get, post},
    Router,
};
use tower_http::services::ServeDir;

use handlers::*;
use middlewares::*;

mod handlers;
mod middlewares;

pub struct _AppState {
    pub tera: tera::Tera,
    pub tera_context: tera::Context,
    pub redis: redis::Client,
}

type AppState = &'static _AppState;

pub fn make_routes(state: _AppState) -> Router {
    let handle_404: fn() -> _ = || async { (StatusCode::NOT_FOUND, "Not found") };
    let serve_dir = ServeDir::new("public").not_found_service(handle_404.into_service());

    let app_state: AppState = Box::leak(Box::new(state));

    let with_banned_users = Router::new()
        .route("/:board/thread", get(new_thread_html))
        .route("/:board/thread", post(new_thread))
        .route("/:board/thread/:parent_thread/comment", post(make_comment))
        .layer(axum::middleware::from_fn_with_state(
            app_state,
            check_for_banned_ips,
        ));

    Router::new()
        .route("/", get(index))
        .route("/rules", get(rules))
        .route(
            "/:board",
            get(board).fallback_service(handle_404.into_service()),
        )
        .route("/:board/:thread_id", get(thread_html))
        .nest_service("/public", serve_dir)
        .merge(with_banned_users)
        .with_state(app_state)
}
