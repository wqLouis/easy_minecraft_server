//! Log management for Minecraft server output.
//!
//! [`LogManager`] stores server stdout in a ring buffer and provides both
//! historical access (tail) and real-time streaming via [`tokio::sync::broadcast`].

use std::collections::VecDeque;

use tokio::sync::broadcast;

/// Default number of log lines kept in memory.
const DEFAULT_MAX_LINES: usize = 10_000;

/// Captures and streams Minecraft server log output.
///
/// # Example
///
/// ```rust,no_run
/// use mc_server_manager::LogManager;
///
/// let mut logs = LogManager::new(1000);
///
/// // Push log lines as they arrive from server stdout
/// logs.push("[14:32:01 INFO]: Done (1.234s)!".into());
///
/// // Read last N lines
/// for line in logs.tail(5) {
///     println!("{line}");
/// }
///
/// // Subscribe to real-time log stream
/// let mut rx = logs.subscribe();
/// ```
#[derive(Debug)]
pub struct LogManager {
    /// Ring buffer of log lines (newest appended at the back).
    buffer: VecDeque<String>,
    /// Maximum number of lines kept in the ring buffer.
    max_lines: usize,
    /// Broadcast channel for real-time log streaming.
    tx: broadcast::Sender<String>,
}

impl LogManager {
    /// Create a new log manager with a given capacity.
    pub fn new(max_lines: usize) -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            buffer: VecDeque::with_capacity(max_lines.min(1024)),
            max_lines,
            tx,
        }
    }

    /// Append a log line. If the buffer exceeds `max_lines`, the oldest
    /// line is evicted.
    pub fn push(&mut self, line: String) {
        // Evict oldest if at capacity
        if self.buffer.len() >= self.max_lines {
            self.buffer.pop_front();
        }
        self.buffer.push_back(line.clone());

        // Broadcast to all subscribers (ignore if no active subscribers)
        let _ = self.tx.send(line);
    }

    /// Return the last `n` log lines (or fewer if not enough history).
    pub fn tail(&self, n: usize) -> Vec<String> {
        let len = self.buffer.len();
        let start = len.saturating_sub(n);
        self.buffer.range(start..).cloned().collect()
    }

    /// Return all buffered log lines (oldest first).
    pub fn all(&self) -> Vec<String> {
        self.buffer.iter().cloned().collect()
    }

    /// Return the number of buffered lines.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Returns `true` if no log lines have been captured yet.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Subscribe to real-time log lines. Each subscriber receives a stream
    /// of all *future* log lines.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    /// Clear all buffered log lines.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Set the maximum number of buffered lines.
    pub fn set_max_lines(&mut self, max: usize) {
        self.max_lines = max;
        while self.buffer.len() > self.max_lines {
            self.buffer.pop_front();
        }
    }
}

impl Default for LogManager {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_LINES)
    }
}

// ---------------------------------------------------------------------------
// Iterator for historical log lines
// ---------------------------------------------------------------------------

impl IntoIterator for LogManager {
    type Item = String;
    type IntoIter = std::collections::vec_deque::IntoIter<String>;

    fn into_iter(self) -> Self::IntoIter {
        self.buffer.into_iter()
    }
}
