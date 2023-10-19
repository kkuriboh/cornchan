use anyhow::Context;
use axum::{
    extract::{Path, State},
    response::{Html, IntoResponse, Redirect},
};
use axum_typed_multipart::{TryFromMultipart, TypedMultipart};
use redis::AsyncCommands;
use tempfile::NamedTempFile;

use crate::{
    data_types::*,
    db::Keys,
    error::{AppError, HtmlResult},
    general_helpers::current_timestamp,
    traits::Id,
};

use super::AppState;

pub async fn index(State(state): State<AppState>) -> anyhow::Result<impl IntoResponse, AppError> {
    Ok(Html(
        state.tera.render("index.tera.html", &state.tera_context)?,
    ))
}

pub async fn rules(State(state): State<AppState>) -> HtmlResult {
    Ok(Html(
        state.tera.render("rules.tera.html", &state.tera_context)?,
    ))
}

pub async fn board(State(state): State<AppState>, Path(board_slug): Path<String>) -> HtmlResult {
    let mut con = state.redis.get_tokio_connection().await?;
    // TODO: 404 on invalid board
    let board = con
        .hget::<_, _, String>(Keys::BOARDS_KEY, format!("board:{board_slug}"))
        .await?;
    let board = serde_json::from_str::<boards::Board>(&board)?;

    // TODO: sorting on the database + cursor pagination
    let mut threads_iter = con
        .hscan_match::<_, _, ((), String)>(Keys::THREADS_KEY, format!("thread:{board_slug}:*"))
        .await?;

    let mut threads = Vec::new();
    while let Some((_, ref thread)) = threads_iter.next_item().await {
        threads.push(serde_json::from_str::<thread::Thread>(thread)?);
    }

    threads.sort_by(|left, right| {
        let extract_timestamp = |x: &thread::Thread| match x {
            thread::Thread::Parent(payload) => payload.timestamp,
            thread::Thread::Comment { payload, .. } => payload.timestamp,
        };
        extract_timestamp(right).cmp(&extract_timestamp(left))
    });

    let mut context = state.tera_context.clone();
    context.insert("board", &board);
    context.insert("threads", &threads);

    let html = state.tera.render("board.tera.html", &context)?;

    Ok(Html(html))
}

pub async fn thread_html(
    State(state): State<AppState>,
    Path((board_slug, thread_id)): Path<(String, u64)>,
) -> HtmlResult {
    let mut con = state.redis.get_tokio_connection().await?;
    let board: String = con
        .hget(Keys::BOARDS_KEY, format!("board:{board_slug}"))
        .await?;
    let board = serde_json::from_str::<boards::Board>(&board)?;

    let (_, thread) = con
        .hscan_match::<_, _, ((), String)>(
            Keys::THREADS_KEY,
            format!("thread:{board_slug}:*:{thread_id}"),
        )
        .await?
        .next_item()
        .await
        .context("thread does not exist")?;
    let thread = serde_json::from_str::<thread::Thread>(&thread)?;

    let mut context = state.tera_context.clone();
    context.insert("board", &board);
    context.insert("thread", &thread);

    let html = state.tera.render("thread.tera.html", &context)?;

    Ok(Html(html))
}

pub async fn new_thread_html(
    State(state): State<AppState>,
    Path(board): Path<String>,
) -> HtmlResult {
    let mut con = state.redis.get_tokio_connection().await?;
    let board: String = con.hget(Keys::BOARDS_KEY, format!("board:{board}")).await?;
    let board = serde_json::from_str::<boards::Board>(&board)?;

    let mut context = state.tera_context.clone();
    context.insert("board", &board);

    let html = state.tera.render("new_thread.tera.html", &context)?;

    Ok(Html(html))
}

#[derive(TryFromMultipart)]
pub struct NewThreadFormBody {
    nickname: String,
    title: String,
    content: String,
    image_1: NamedTempFile,
    image_2: NamedTempFile,
    image_3: NamedTempFile,
}

pub async fn new_thread(
    State(state): State<AppState>,
    Path(board): Path<String>,
    TypedMultipart(data): TypedMultipart<NewThreadFormBody>,
) -> anyhow::Result<Redirect, AppError> {
    let paths = persist_thread_images([data.image_1, data.image_2, data.image_3]).await?;

    let mut con = state.redis.get_tokio_connection().await?;
    let id: u64 = con.incr(Keys::POST_COUNT_KEY, 1u64).await?;

    let thread = thread::Thread::Parent(thread::ThreadPayload {
        id,
        nickname: if data.nickname.is_empty() {
            String::from("Anonymous")
        } else {
            data.nickname
        },
        title: data.title,
        content: data.content,
        board: board.clone(),
        timestamp: current_timestamp(),
        image_1: paths.get(0).cloned(),
        image_2: paths.get(1).cloned(),
        image_3: paths.get(2).cloned(),
    });

    con.hset(
        Keys::THREADS_KEY,
        thread.ident(),
        serde_json::to_string(&thread)?,
    )
    .await?;

    Ok(Redirect::to(&format!("/{board}")))
}

pub async fn make_comment(
    State(state): State<AppState>,
    Path((board, parent_thread)): Path<(String, u64)>,
    TypedMultipart(data): TypedMultipart<NewThreadFormBody>,
) -> anyhow::Result<Redirect, AppError> {
    let paths = persist_thread_images([data.image_1, data.image_2, data.image_3]).await?;

    let mut con = state.redis.get_tokio_connection().await?;
    let id: u64 = con.incr(Keys::POST_COUNT_KEY, 1u64).await?;

    let thread = thread::Thread::Comment {
        parent_thread,
        payload: thread::ThreadPayload {
            id,
            nickname: if data.nickname.is_empty() {
                String::from("Anonymous")
            } else {
                data.nickname
            },
            title: data.title,
            content: data.content,
            board: board.clone(),
            timestamp: current_timestamp(),
            image_1: paths.get(0).cloned(),
            image_2: paths.get(1).cloned(),
            image_3: paths.get(2).cloned(),
        },
    };

    con.hset(
        Keys::THREADS_KEY,
        thread.ident(),
        serde_json::to_string(&thread)?,
    )
    .await?;

    Ok(Redirect::to(&format!("/{board}/{parent_thread}")))
}

async fn persist_thread_images(
    images: [NamedTempFile; 3],
) -> anyhow::Result<Vec<String>, AppError> {
    use std::io::{BufReader, Read, Seek, SeekFrom};

    const IMAGE_SIZE_THRESHOLD: u64 = 1024 * 200; // 200KB
    let mut paths = Vec::with_capacity(3);

    for image in images {
        let image_size = image.as_file().metadata()?.len();
        if image_size < 32 {
            continue;
        }

        let mut image = BufReader::new(image);

        let mut format_buffer = [0; 32];
        unsafe {
            // SAFETY: file size is already checked
            image.read_exact(&mut format_buffer).unwrap_unchecked();
        }
        let image_format = image::guess_format(&format_buffer).context("invalid image format")?;

        image.seek(SeekFrom::Start(0))?;
        let image = image::load(image, image_format)?;

        let new_path = format!("public/{}", nanoid::nanoid!());
        let f = std::fs::File::create(&new_path)?;

        let compression = if image_size >= IMAGE_SIZE_THRESHOLD {
            image::codecs::webp::WebPQuality::lossy(50)
        } else {
            image::codecs::webp::WebPQuality::default()
        };

        let encoder = image::codecs::webp::WebPEncoder::new_with_quality(f, compression);

        // TODO: save the images somewhere else
        image
            .write_with_encoder(encoder)
            .context("failed to save file")?;

        paths.push(new_path);
    }

    Ok(paths)
}
