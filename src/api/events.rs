use std::convert::Infallible;

use async_stream::stream;
use axum::{
    extract::State,
    http::header,
    response::sse::{Event, Sse},
    response::{sse::KeepAlive, IntoResponse, Response},
};
use chrono::Utc;

use crate::{models::ServerEvent, state::AppState};

pub async fn events(State(state): State<AppState>) -> Response {
    let mut receiver = state.events.subscribe();
    let hello = serde_json::json!({
        "time": Utc::now().to_rfc3339(),
        "version": state.version,
    });
    let stream = stream! {
        yield Ok::<Event, Infallible>(Event::default().event("hello").data(hello.to_string()));
        loop {
            match receiver.recv().await {
                Ok(event) => {
                    match event {
                        ServerEvent::Task { task } => {
                            if let Ok(json) = serde_json::to_string(&task) {
                                yield Ok(Event::default().event("task").data(json));
                            }
                        }
                        ServerEvent::TaskDeleted { task_id } => {
                            yield Ok(Event::default().event("deleted").data(task_id));
                        }
                        ServerEvent::Hello { time } => {
                            yield Ok(Event::default().event("hello").data(time));
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    yield Ok(Event::default().event("resync").data("{}"));
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    let mut response = Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(std::time::Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-cache".parse().unwrap());
    response
        .headers_mut()
        .insert("X-Accel-Buffering", "no".parse().unwrap());
    response
}
