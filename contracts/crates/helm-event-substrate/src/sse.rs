//! SSE (Server-Sent Events) transport for the event substrate.
//!
//! Provides an axum router, a raw frame encoder, and a generic
//! replay-then-live stream combinator.
//!
//! Moved verbatim from `runway-app-host/src/sse.rs` (RFL-171 T2).
//! Imports retargeted to crate types: `crate::event` for envelope/subscription,
//! `crate::hub` for the handle.
//!
//! Gated behind the `sse` feature (default on).

use std::convert::Infallible;

use axum::{
    Router,
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    routing::get,
};
use futures::stream::{Stream, StreamExt};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

use crate::event::{EventEnvelope, EventSubscription};
use crate::hub::EventHubHandle;

pub fn router(hub: EventHubHandle) -> Router {
    Router::new()
        .route("/sse/stream", get(stream))
        .with_state(hub)
}

async fn stream(
    State(hub): State<EventHubHandle>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = hub.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|ev| async move {
        ev.ok().and_then(|env| {
            serde_json::to_string(&env)
                .ok()
                .map(|s| Ok(Event::default().event(env.r#type.clone()).data(s)))
        })
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Encode an envelope as an SSE frame (`id` = sequence, `data` = JSON).
///
/// Returns `None` when the envelope cannot be serialized (matches prior helm
/// call-site behavior).
#[must_use]
pub fn encode_frame(env: &EventEnvelope) -> Option<Event> {
    serde_json::to_string(env)
        .ok()
        .map(|data| Event::default().id(env.sequence.to_string()).data(data))
}

/// Replay the subscription buffer, then stream live events — deduped by
/// sequence, tolerant of lag. `filter` selects envelopes to yield; `terminal`
/// (checked only on yielded envelopes) decides when to stop.
pub fn event_stream<F, T>(
    subscription: EventSubscription,
    filter: F,
    terminal: T,
) -> impl tokio_stream::Stream<Item = Result<Event, Infallible>>
where
    F: Fn(&EventEnvelope) -> bool,
    T: Fn(&EventEnvelope) -> bool,
{
    async_stream::stream! {
        let mut last_sequence = 0u64;
        for env in subscription.replay {
            last_sequence = env.sequence;
            if filter(&env) {
                let stop = terminal(&env);
                if let Some(frame) = encode_frame(&env) {
                    yield Ok(frame);
                }
                if stop {
                    return;
                }
            }
        }
        let mut live = subscription.receiver;
        loop {
            match live.recv().await {
                Ok(env) => {
                    if env.sequence <= last_sequence {
                        continue;
                    }
                    last_sequence = env.sequence;
                    if filter(&env) {
                        let stop = terminal(&env);
                        if let Some(frame) = encode_frame(&env) {
                            yield Ok(frame);
                        }
                        if stop {
                            break;
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventCursor;
    use crate::hub::EventHub;
    use axum::body::Body;
    use axum::http::Request;
    use chrono::Utc;
    use futures::StreamExt;
    use tower::ServiceExt;
    use uuid::Uuid;

    fn sample(seq: u64, ty: &str, run_id: Option<&str>) -> EventEnvelope {
        EventEnvelope {
            event_id: Uuid::new_v4(),
            sequence: seq,
            r#type: ty.into(),
            schema_version: 1,
            occurred_at: Utc::now(),
            app_id: "test".into(),
            run_id: run_id.map(String::from),
            job_id: None,
            correlation_id: None,
            actor: None,
            payload: serde_json::Value::Null,
        }
    }

    #[tokio::test]
    async fn sse_endpoint_is_reachable() {
        let hub = EventHub::new();
        let app = router(hub.handle());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/sse/stream")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let ct = resp.headers().get("content-type").unwrap();
        assert!(ct.to_str().unwrap().starts_with("text/event-stream"));
    }

    #[tokio::test]
    async fn event_stream_replay_filters_and_terminates() {
        let hub = EventHub::with_capacity(8);
        let h = hub.handle();
        h.publish(sample(0, "job.started", Some("run-1")));
        h.publish(sample(0, "job.progress", Some("run-2")));
        h.publish(sample(0, "job.completed", Some("run-1")));

        let sub = h
            .subscribe_with_cursor(EventCursor {
                run_id: Some("run-1".into()),
                ..Default::default()
            })
            .await;

        let run_id = "run-1".to_string();
        let mut stream = Box::pin(event_stream(
            sub,
            move |env| env.run_id.as_deref() == Some(run_id.as_str()),
            |env| env.r#type == "job.completed",
        ));

        assert!(stream.next().await.is_some());
        assert!(stream.next().await.is_some());
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn event_stream_live_dedups_by_sequence() {
        let hub = EventHub::with_capacity(8);
        let h = hub.handle();
        let sub = h
            .subscribe_with_cursor(EventCursor::default())
            .await;
        let mut stream = Box::pin(event_stream(sub, |_| true, |_| false));
        let publish = h.clone();
        tokio::spawn(async move {
            publish.publish(sample(0, "a", None));
            publish.publish(sample(0, "b", None));
        });
        assert!(stream.next().await.is_some());
        assert!(stream.next().await.is_some());
    }
}
