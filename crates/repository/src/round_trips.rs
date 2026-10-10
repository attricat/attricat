//! Database round-trip accounting for one request or background operation.
//!
//! A scope is a task-local counter. The tracing layer installed by the HTTP
//! telemetry module calls [`record_round_trip`] for every completed SQLx
//! statement (including `BEGIN`/`COMMIT`), so the count reflects network round
//! trips rather than repository calls. Statements run outside a scope, or in a
//! task spawned from one, are reported under the `unscoped` label.
//!
//! On completion a scope records `attricat_db_round_trips_per_operation` (total
//! statements, including nested scopes), adds its own statements to
//! `attricat_db_round_trips_total`, and emits a debug log line.

use std::{
    borrow::Cow,
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

tokio::task_local! {
    static CURRENT: Arc<Scope>;
}

struct Scope {
    name: Cow<'static, str>,
    /// Statements issued while this was the innermost scope.
    own: AtomicU64,
    /// Statements issued in this scope or any nested scope.
    total: AtomicU64,
    parent: Option<Arc<Scope>>,
}

/// Reports the scope even when the measured future is cancelled.
struct Report(Arc<Scope>);

impl Drop for Report {
    fn drop(&mut self) {
        let scope = &self.0;
        let own = scope.own.load(Ordering::Relaxed);
        let total = scope.total.load(Ordering::Relaxed);
        metrics::counter!("attricat_db_round_trips_total", "scope" => scope.name.to_string())
            .increment(own);
        metrics::histogram!(
            "attricat_db_round_trips_per_operation",
            "scope" => scope.name.to_string()
        )
        .record(total as f64);
        if total > 0 {
            tracing::debug!(scope = %scope.name, round_trips = total, "database round trips");
        }
    }
}

/// Runs `future` in a named round-trip scope. Nested scopes also count toward
/// every enclosing scope's total.
pub async fn measure<F: Future>(name: impl Into<Cow<'static, str>>, future: F) -> F::Output {
    let scope = Arc::new(Scope {
        name: name.into(),
        own: AtomicU64::new(0),
        total: AtomicU64::new(0),
        parent: CURRENT.try_with(Arc::clone).ok(),
    });
    let _report = Report(scope.clone());
    CURRENT.scope(scope, future).await
}

/// Counts one completed database statement against the current scope.
pub fn record_round_trip() {
    let scoped = CURRENT.try_with(|scope| {
        scope.own.fetch_add(1, Ordering::Relaxed);
        let mut current = Some(scope.as_ref());
        while let Some(scope) = current {
            scope.total.fetch_add(1, Ordering::Relaxed);
            current = scope.parent.as_deref();
        }
    });
    if scoped.is_err() {
        metrics::counter!("attricat_db_round_trips_total", "scope" => "unscoped").increment(1);
    }
}

/// The current scope's total so far, or `None` outside a scope.
pub fn current_round_trips() -> Option<u64> {
    CURRENT
        .try_with(|scope| scope.total.load(Ordering::Relaxed))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn nested_scopes_roll_up_totals() {
        assert_eq!(current_round_trips(), None);
        let (outer, inner) = measure("outer", async {
            record_round_trip();
            let inner = measure("inner", async {
                record_round_trip();
                record_round_trip();
                current_round_trips()
            })
            .await;
            (current_round_trips(), inner)
        })
        .await;
        assert_eq!(inner, Some(2));
        assert_eq!(outer, Some(3));
    }
}
