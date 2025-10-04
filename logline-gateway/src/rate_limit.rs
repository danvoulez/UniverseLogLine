use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use tracing::warn;

#[derive(Debug)]
struct RateWindow {
    started_at: Instant,
    count: u64,
}

#[derive(Debug, Clone)]
pub struct RateLimitState {
    limit: u64,
    period: Duration,
    window: Arc<Mutex<RateWindow>>,
}

impl RateLimitState {
    pub fn new(limit: u64, period: Duration) -> Self {
        let now = Instant::now();
        Self {
            limit,
            period,
            window: Arc::new(Mutex::new(RateWindow {
                started_at: now,
                count: 0,
            })),
        }
    }

    pub fn unlimited() -> Self {
        Self::new(u64::MAX, Duration::from_secs(60))
    }

    pub fn try_acquire(&self) -> bool {
        if self.limit == 0 {
            return true;
        }

        let mut window = self.window.lock().expect("rate limit mutex poisoned");
        let now = Instant::now();

        if now.duration_since(window.started_at) >= self.period {
            window.started_at = now;
            window.count = 0;
        }

        if window.count < self.limit {
            window.count += 1;
            true
        } else {
            false
        }
    }
}

pub async fn enforce_rate_limit(
    State(state): State<Arc<RateLimitState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if state.try_acquire() {
        Ok(next.run(request).await)
    } else {
        warn!("taxa de requisições excedida");
        Err(StatusCode::TOO_MANY_REQUESTS)
    }
}
