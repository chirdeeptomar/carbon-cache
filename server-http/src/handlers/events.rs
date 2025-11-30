use crate::state::AppState;
use axum::{
    extract::State,
    http::Uri,
    response::sse::{Event, KeepAlive, Sse},
};
use carbon::events::CacheItemEvent;
use futures::stream::{Stream, StreamExt};
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;

#[derive(Clone, Debug)]
pub struct EventFilter {
    cache: Vec<String>,
    event_type: Vec<String>,
}

impl EventFilter {
    /// Parse query string with CSV support for multiple values
    /// Examples: ?cache=cache1,cache2&type=added,updated
    fn from_query_string(query: &str) -> Self {
        let mut cache = Vec::new();
        let mut event_type = Vec::new();

        for pair in query.split('&') {
            if let Some((key, value)) = pair.split_once('=') {
                match key {
                    "cache" => {
                        cache.extend(value.split(',').map(|s| s.trim().to_string()));
                    }
                    "type" => {
                        event_type.extend(value.split(',').map(|s| s.trim().to_string()));
                    }
                    _ => {}
                }
            }
        }

        Self { cache, event_type }
    }
}

/// SSE endpoint that streams cache item events to clients
pub async fn stream_events(
    State(state): State<AppState>,
    uri: Uri,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let filter = uri
        .query()
        .map(EventFilter::from_query_string)
        .unwrap_or_else(|| EventFilter {
            cache: Vec::new(),
            event_type: Vec::new(),
        });

    tracing::info!(
        "New SSE client connected. Filters: cache={:?}, type={:?}",
        filter.cache,
        filter.event_type
    );

    let rx = state.event_channel.subscribe();
    let stream = BroadcastStream::new(rx);

    let filtered_stream = stream.filter_map(move |result| {
        let filter_clone = filter.clone();
        async move {
            match result {
                Ok(event) => {
                    let should_send_event = should_send(&event, &filter_clone);
                    tracing::debug!(
                        "Received event: cache={}, key={:?}, should_send={}",
                        event.cache_name(),
                        String::from_utf8_lossy(event.key()),
                        should_send_event
                    );
                    if should_send_event {
                        Some(Ok(to_sse_event(event)))
                    } else {
                        None
                    }
                }
                Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(n)) => {
                    Some(Ok(Event::default()
                        .event("error")
                        .data(format!("Lagged by {} events", n))))
                }
            }
        }
    });

    Sse::new(filtered_stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

/// Check if an event should be sent based on the filter criteria
fn should_send(event: &CacheItemEvent, filter: &EventFilter) -> bool {
    // Filter by cache name (only if filter is specified)
    if !filter.cache.is_empty() && !filter.cache.iter().any(|c| c == event.cache_name()) {
        return false;
    }

    // Filter by event type (only if filter is specified)
    if !filter.event_type.is_empty() {
        let event_type_str = match event {
            CacheItemEvent::Added(_) => "added",
            CacheItemEvent::Updated(_) => "updated",
            CacheItemEvent::Deleted(_) => "deleted",
        };

        if !filter.event_type.iter().any(|t| t == event_type_str) {
            return false;
        }
    }

    true
}

/// Convert a CacheItemEvent to an SSE Event
fn to_sse_event(event: CacheItemEvent) -> Event {
    match event {
        CacheItemEvent::Added(e) => Event::default().event("item.added").json_data(e).unwrap(),
        CacheItemEvent::Updated(e) => Event::default().event("item.updated").json_data(e).unwrap(),
        CacheItemEvent::Deleted(e) => Event::default().event("item.deleted").json_data(e).unwrap(),
    }
}
