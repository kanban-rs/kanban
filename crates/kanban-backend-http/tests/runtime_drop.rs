use kanban_backend::KanbanBackend;
use kanban_backend_http::HttpBackend;
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread")]
async fn test_dropping_the_http_backend_inside_an_async_context_does_not_panic() {
    let backend = HttpBackend::new("http://127.0.0.1:1").unwrap();
    drop(backend);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_dropping_the_last_arc_to_the_http_backend_inside_an_async_context_does_not_panic() {
    let backend: Arc<dyn KanbanBackend> = Arc::new(HttpBackend::new("http://127.0.0.1:1").unwrap());
    let clone = Arc::clone(&backend);
    drop(backend);
    drop(clone);
}

#[test]
fn test_dropping_the_http_backend_outside_any_runtime_does_not_panic() {
    let backend = HttpBackend::new("http://127.0.0.1:1").unwrap();
    drop(backend);
}
