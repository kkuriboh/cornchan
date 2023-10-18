use std::net::SocketAddr;

use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::Request,
    middleware::Next,
    response::IntoResponse,
};
use redis::AsyncCommands;
use sha3::Digest;

use crate::{db::Keys, error::AppError, general_helpers::current_timestamp};

use super::AppState;

pub async fn check_for_banned_ips(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    request: Request<Body>,
    next: Next<Body>,
) -> Result<impl IntoResponse, AppError> {
    let mut hasher = sha3::Keccak224::new();
    let ip = addr.ip().to_string();
    hasher.update(ip.as_bytes());
    let hashed_ip: [u8; 28] = hasher.finalize().into();

    let mut con = state.redis.get_tokio_connection().await?;

    let ban_release_timestamp = con
        .hget::<_, _, Option<u64>>("banned_ips", &hashed_ip as &[u8])
        .await?;

    match ban_release_timestamp {
        Some(ban_release_timestamp) if ban_release_timestamp > current_timestamp() => {
            return Err(anyhow::anyhow!("you're currently banned from cornchan").into())
        }
        Some(_) => {
            con.hdel(Keys::BANNED_IPS_KEY, &hashed_ip as &[u8]).await?;
        }
        None => {}
    }

    Ok(next.run(request).await)
}
