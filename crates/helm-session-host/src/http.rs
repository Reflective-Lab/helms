// Copyright 2024-2026 Reflective Labs
// SPDX-License-Identifier: MIT

//! HTTP adapter — session-scoped SSE over upstream `runway_app_host::sse`.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::get;
use runway_app_host::{EventCursor, EventSubscription};

use crate::service::SessionHostService;

/// Build the session-host router.
pub fn router(service: Arc<SessionHostService>) -> Router {
    Router::new()
        .route("/v1/sessions/{session_id}/stream", get(stream))
        .with_state(service)
}

async fn stream(
    State(service): State<Arc<SessionHostService>>,
    Path(session_id): Path<String>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let subscription = service
        .hub()
        .subscribe_with_cursor(EventCursor::default())
        .await;
    let stream = build_stream(subscription, session_id);
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

fn build_stream(
    subscription: EventSubscription,
    session_id: String,
) -> impl tokio_stream::Stream<Item = Result<Event, Infallible>> {
    runway_app_host::sse::event_stream(
        subscription,
        move |env| SessionHostService::stream_includes(env, &session_id),
        |_| false,
    )
}
