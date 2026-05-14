//! Player tracking by parsing Minecraft server log output.
//!
//! [`PlayerTracker`] monitors log lines for join/quit events and maintains
//! a real-time list of online players.

use std::collections::HashMap;

use chrono::Utc;
use regex::Regex;

/// Information about a player who has connected to the server.
#[derive(Debug, Clone)]
pub struct PlayerInfo {
    /// In-game username.
    pub name: String,
    /// UUID if it could be parsed from log output.
    pub uuid: Option<String>,
    /// When the player joined (UTC).
    pub joined_at: chrono::DateTime<Utc>,
}

/// Tracks online players by parsing Minecraft server log output.
///
/// # What it parses
///
/// | Event | Log pattern |
/// |-------|-------------|
/// | Join  | `XXX joined the game` |
/// | Leave | `XXX left the game` |
/// | UUID  | `UUID of player XXX is xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx` |
/// | List  | `There are N of max players online:` (optional) |
///
/// # Example
///
/// ```rust,no_run
/// use mc_server_manager::PlayerTracker;
///
/// let mut tracker = PlayerTracker::new();
///
/// tracker.process_log_line("[14:32:01 INFO]: UUID of player Steve is 123e4567-e89b-12d3-a456-426614174000");
/// tracker.process_log_line("[14:32:05 INFO]: Steve joined the game");
///
/// assert_eq!(tracker.player_count(), 1);
/// println!("Online: {:?}", tracker.online_players());
/// ```
#[derive(Debug)]
pub struct PlayerTracker {
    online: HashMap<String, PlayerInfo>,
    join_re: Regex,
    leave_re: Regex,
    uuid_re: Regex,
    #[allow(dead_code)]
    list_re: Regex,
}

impl PlayerTracker {
    /// Create a new tracker with compiled regex patterns.
    pub fn new() -> Self {
        Self {
            online: HashMap::new(),
            // "Steve joined the game"
            join_re: Regex::new(r#"^(?P<name>\w{3,16}) joined the game$"#).unwrap(),
            // "Steve left the game"
            leave_re: Regex::new(r#"^(?P<name>\w{3,16}) left the game$"#).unwrap(),
            // "UUID of player Steve is 123e4567-e89b-12d3-a456-426614174000"
            uuid_re: Regex::new(
                r#"^UUID of player (?P<name>\w{3,16}) is (?P<uuid>[0-9a-f\-]{36})$"#,
            )
            .unwrap(),
            // "There are 1 of 20 players online:"
            list_re: Regex::new(r#"^There are (?P<count>\d+) of \d+ players online:"#).unwrap(),
        }
    }

    /// Process a single log line and update player state.
    ///
    /// Call this for every line coming from the server's stdout.
    pub fn process_log_line(&mut self, line: &str) {
        // Strip timestamp prefix like "[14:32:01 INFO]: "
        let msg = if let Some(content) = line.split("]: ").nth(1) {
            content
        } else {
            line
        };

        // Check patterns in order of likelihood
        if let Some(caps) = self.join_re.captures(msg) {
            let name = caps["name"].to_string();
            if !self.online.contains_key(&name) {
                let info = PlayerInfo {
                    name: name.clone(),
                    uuid: None,
                    joined_at: Utc::now(),
                };
                self.online.insert(name, info);
            }
            return;
        }

        if let Some(caps) = self.leave_re.captures(msg) {
            let name = caps["name"].to_string();
            self.online.remove(&name);
            return;
        }

        if let Some(caps) = self.uuid_re.captures(msg) {
            let name = caps["name"].to_string();
            let uuid = caps["uuid"].to_string();
            if let Some(info) = self.online.get_mut(&name) {
                info.uuid = Some(uuid);
            }
            return;
        }

        // list_re is informational only, no state change needed
    }

    /// Return the list of currently online player names (sorted
    /// alphabetically).
    pub fn online_players(&self) -> Vec<String> {
        let mut names: Vec<String> = self.online.keys().cloned().collect();
        names.sort();
        names
    }

    /// Return the number of currently online players.
    pub fn player_count(&self) -> usize {
        self.online.len()
    }

    /// Get detailed info for a specific player.
    pub fn player_info(&self, name: &str) -> Option<&PlayerInfo> {
        self.online.get(name)
    }

    /// Check if a specific player is online.
    pub fn is_online(&self, name: &str) -> bool {
        self.online.contains_key(name)
    }

    /// Clear all tracked players (e.g. on server restart).
    pub fn clear(&mut self) {
        self.online.clear();
    }
}

impl Default for PlayerTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_join_leave() {
        let mut t = PlayerTracker::new();
        t.process_log_line("[14:32:01 INFO]: Steve joined the game");
        assert_eq!(t.player_count(), 1);
        assert!(t.is_online("Steve"));

        t.process_log_line("[14:32:05 INFO]: Steve left the game");
        assert_eq!(t.player_count(), 0);
    }

    #[test]
    fn test_uuid_tracking() {
        let mut t = PlayerTracker::new();
        t.process_log_line("[14:32:01 INFO]: Steve joined the game");
        t.process_log_line(
            "[14:32:02 INFO]: UUID of player Steve is 123e4567-e89b-12d3-a456-426614174000",
        );

        let info = t.player_info("Steve").unwrap();
        assert_eq!(
            info.uuid.as_deref(),
            Some("123e4567-e89b-12d3-a456-426614174000")
        );
    }

    #[test]
    fn test_multiple_players() {
        let mut t = PlayerTracker::new();
        t.process_log_line("[14:32:01 INFO]: Alice joined the game");
        t.process_log_line("[14:32:05 INFO]: Bob joined the game");
        assert_eq!(t.player_count(), 2);

        let players = t.online_players();
        assert_eq!(players, vec!["Alice", "Bob"]);
    }
}
