use redis::AsyncCommands;

use routes::*;

mod data_types;
mod db;
mod error;
mod general_helpers;
mod routes;
mod traits;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let tera = tera::Tera::new("views/*")?;
    let mut tera_context = tera::Context::new();
    tera_context.insert("title", "cornchan");

    let redis = redis::Client::open("redis://localhost:6379")?;

    let mut con = redis.get_tokio_connection().await?;
    use traits::Id;
    let board = data_types::boards::test_board();
    con.hset(
        db::Keys::BOARDS_KEY,
        board.ident(),
        serde_json::to_string(&board)?,
    )
    .await?;

    let boards = con
        .hgetall::<_, Vec<(String, String)>>(db::Keys::BOARDS_KEY)
        .await?
        .into_iter()
        .map(|(_, b)| serde_json::from_str::<data_types::boards::Board>(&b).unwrap())
        .collect::<Box<[_]>>();
    tera_context.insert("boards", &boards);

    drop(con); // con would live forver and reduce the amount of connections that can be pooled

    let app_state = _AppState {
        tera,
        tera_context,
        redis,
    };

    let router = make_routes(app_state);

    tracing::info!("SERVER RUNNING AT PORT 3000");
    axum::Server::bind(&"[::]:3000".parse()?)
        .serve(router.into_make_service_with_connect_info::<std::net::SocketAddr>())
        .await?;

    Ok(())
}
