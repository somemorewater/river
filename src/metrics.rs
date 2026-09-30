use std::sync::atomic::{AtomicUsize, Ordering};

/// In-memory operational counters shared by the server.
///
/// Everything is an `AtomicUsize` so any Tokio task can record without
/// locking. All counting is best-effort: exact enough for operators, never
/// on a correctness path. Owned by `ConcurrentStore` so every layer that
/// already holds the store can record without new plumbing.
#[derive(Debug, Default)]
pub struct Metrics {
    /// Every command dispatched, including STATS/HEALTH/EXIT and rejections.
    pub commands_total: AtomicUsize,
    /// Per-command dispatch counts (accepted syntax, before execution).
    pub cmd_set: AtomicUsize,
    pub cmd_get: AtomicUsize,
    pub cmd_del: AtomicUsize,
    pub cmd_expire: AtomicUsize,
    pub cmd_setex: AtomicUsize,
    pub cmd_ping: AtomicUsize,
    pub cmd_stats: AtomicUsize,
    pub cmd_health: AtomicUsize,
    pub cmd_exit: AtomicUsize,
    pub cmd_auth: AtomicUsize,
    /// Successful AUTH commands.
    pub auth_success: AtomicUsize,
    /// Failed AUTH attempts (wrong password or AUTH while disabled).
    pub auth_failures: AtomicUsize,
    /// Commands rejected because the connection is not authenticated.
    pub auth_required: AtomicUsize,
    /// Unknown commands + invalid syntax + failed executions.
    pub command_errors: AtomicUsize,
    /// Malformed RESP frames and non-command frame shapes on the wire.
    pub protocol_errors: AtomicUsize,
    /// Currently open client connections (gauge: inc on accept, dec on close).
    pub connections_active: AtomicUsize,
    /// Total connections accepted since startup.
    pub connections_total: AtomicUsize,
    /// Successful snapshot writes after mutations/cleanups.
    pub persistence_saves: AtomicUsize,
    /// Failed snapshot writes.
    pub persistence_failures: AtomicUsize,
    /// Keys removed because their TTL elapsed (any path).
    pub expired_keys: AtomicUsize,
    /// Background TTL cleanup cycles that ran.
    pub cleanup_runs: AtomicUsize,
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }

    fn inc(counter: &AtomicUsize) {
        counter.fetch_add(1, Ordering::Relaxed);
    }

    fn add(counter: &AtomicUsize, n: usize) {
        counter.fetch_add(n, Ordering::Relaxed);
    }

    pub fn record_command(&self, name: &str) {
        Self::inc(&self.commands_total);
        let counter = match name {
            "SET" => &self.cmd_set,
            "GET" => &self.cmd_get,
            "DEL" => &self.cmd_del,
            "EXPIRE" => &self.cmd_expire,
            "SETEX" => &self.cmd_setex,
            "PING" => &self.cmd_ping,
            "STATS" => &self.cmd_stats,
            "HEALTH" => &self.cmd_health,
            "EXIT" => &self.cmd_exit,
            "AUTH" => &self.cmd_auth,
            _ => return,
        };
        Self::inc(counter);
    }

    pub fn record_command_error(&self) {
        Self::inc(&self.commands_total);
        Self::inc(&self.command_errors);
    }

    /// A dispatched command that failed during execution (already counted in
    /// `commands_total` by `record_command`).
    pub fn record_failed_execution(&self) {
        Self::inc(&self.command_errors);
    }

    pub fn record_auth_success(&self) {
        Self::inc(&self.auth_success);
    }

    pub fn record_auth_failure(&self) {
        Self::inc(&self.auth_failures);
    }

    pub fn record_auth_required(&self) {
        Self::inc(&self.auth_required);
    }

    pub fn record_protocol_error(&self) {
        Self::inc(&self.protocol_errors);
    }

    pub fn connection_opened(&self) {
        Self::inc(&self.connections_total);
        Self::inc(&self.connections_active);
    }

    pub fn connection_closed(&self) {
        self.connections_active.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn record_persistence_saved(&self) {
        Self::inc(&self.persistence_saves);
    }

    pub fn record_persistence_failed(&self) {
        Self::inc(&self.persistence_failures);
    }

    pub fn record_expired(&self, n: usize) {
        Self::add(&self.expired_keys, n);
    }

    pub fn record_cleanup_run(&self) {
        Self::inc(&self.cleanup_runs);
    }

    pub fn load(counter: &AtomicUsize) -> usize {
        counter.load(Ordering::Relaxed)
    }
}

/// RAII guard: open a connection count on creation, close it on drop, so
/// every exit path (EOF, protocol error, disconnect, shutdown) decrements.
pub struct ActiveConnectionGuard<'a> {
    metrics: &'a Metrics,
}

impl<'a> ActiveConnectionGuard<'a> {
    pub fn open(metrics: &'a Metrics) -> Self {
        metrics.connection_opened();
        Self { metrics }
    }
}

impl Drop for ActiveConnectionGuard<'_> {
    fn drop(&mut self) {
        self.metrics.connection_closed();
    }
}

#[cfg(test)]
mod tests {
    use super::{ActiveConnectionGuard, Metrics};

    #[test]
    fn counters_increment() {
        let m = Metrics::new();
        m.record_command("SET");
        m.record_command("SET");
        m.record_command("GET");
        m.record_command_error();
        m.record_protocol_error();
        m.record_persistence_saved();
        m.record_persistence_failed();
        m.record_expired(3);
        m.record_cleanup_run();
        m.record_command("AUTH");
        m.record_auth_success();
        m.record_auth_failure();
        m.record_auth_required();

        assert_eq!(Metrics::load(&m.commands_total), 5);
        assert_eq!(Metrics::load(&m.cmd_set), 2);
        assert_eq!(Metrics::load(&m.cmd_get), 1);
        assert_eq!(Metrics::load(&m.command_errors), 1);
        assert_eq!(Metrics::load(&m.protocol_errors), 1);
        assert_eq!(Metrics::load(&m.persistence_saves), 1);
        assert_eq!(Metrics::load(&m.persistence_failures), 1);
        assert_eq!(Metrics::load(&m.expired_keys), 3);
        assert_eq!(Metrics::load(&m.cleanup_runs), 1);
        assert_eq!(Metrics::load(&m.cmd_auth), 1);
        assert_eq!(Metrics::load(&m.auth_success), 1);
        assert_eq!(Metrics::load(&m.auth_failures), 1);
        assert_eq!(Metrics::load(&m.auth_required), 1);
    }

    #[test]
    fn guard_returns_active_to_zero() {
        let m = Metrics::new();
        {
            let _a = ActiveConnectionGuard::open(&m);
            assert_eq!(Metrics::load(&m.connections_active), 1);
            {
                let _b = ActiveConnectionGuard::open(&m);
                assert_eq!(Metrics::load(&m.connections_active), 2);
            }
            assert_eq!(Metrics::load(&m.connections_active), 1);
        }
        assert_eq!(Metrics::load(&m.connections_active), 0);
        assert_eq!(Metrics::load(&m.connections_total), 2);
    }
}
