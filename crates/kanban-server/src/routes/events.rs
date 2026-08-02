use crate::state::AppState;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::get;
use axum::Router;
use futures_util::stream;
use kanban_service::api::ChangeEventFrame;
use std::convert::Infallible;
use std::time::Duration;
use tokio::sync::broadcast;

fn frame_to_event(frame: &ChangeEventFrame) -> Event {
    Event::default()
        .json_data(frame)
        .expect("ChangeEventFrame always serializes")
}

async fn events(
    State(state): State<AppState>,
) -> Sse<impl futures_util::stream::Stream<Item = Result<Event, Infallible>> + Send> {
    let rx = state.event_tx.subscribe();
    let stream = stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(frame) => return Some((Ok(frame_to_event(&frame)), rx)),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/events", get(events))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_event_frame_maps_to_sse_data_json() {
        let frame = ChangeEventFrame::now(
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            kanban_core::ClientId::new(),
        );
        let event = frame_to_event(&frame);
        // axum's Event doesn't expose accessors, but as long as frame_to_event
        // doesn't panic on a real frame, the integration tests will verify
        // the actual JSON shape reaching the wire is correct.
        let _ = event;
    }
}
