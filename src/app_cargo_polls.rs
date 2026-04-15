use super::App;
use std::time::{Duration, SystemTime};

pub(crate) const CARGO_HASH_POLL_INTERVAL: Duration = Duration::from_secs(60);
pub(crate) const CARGO_HASH_POLL_TIMEOUT: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone)]
pub(crate) struct CargoHashPoll {
    pub repo_name: String,
    pub started_at: SystemTime,
    pub next_check_at: SystemTime,
    pub in_flight: bool,
    pub after_auto_update: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExpiredCargoHashPoll {
    pub repo_name: String,
    pub after_auto_update: bool,
}

impl App {
    pub(crate) fn start_cargo_hash_polling(&mut self, repo_name: &str) {
        self.start_cargo_hash_polling_at(repo_name, SystemTime::now());
    }

    pub(crate) fn start_auto_update_cargo_hash_polling(&mut self, repo_name: &str) {
        self.start_cargo_hash_polling_with_source_at(repo_name, SystemTime::now(), true);
    }

    pub(crate) fn start_cargo_hash_polling_at(&mut self, repo_name: &str, now: SystemTime) {
        self.start_cargo_hash_polling_with_source_at(repo_name, now, false);
    }

    fn start_cargo_hash_polling_with_source_at(
        &mut self,
        repo_name: &str,
        now: SystemTime,
        after_auto_update: bool,
    ) {
        let next_check_at = now + CARGO_HASH_POLL_INTERVAL;
        if let Some(poll) = self
            .cargo_hash_polls
            .iter_mut()
            .find(|poll| poll.repo_name == repo_name)
        {
            poll.started_at = now;
            poll.next_check_at = next_check_at;
            poll.in_flight = false;
            poll.after_auto_update |= after_auto_update;
        } else {
            self.cargo_hash_polls.push(CargoHashPoll {
                repo_name: repo_name.to_string(),
                started_at: now,
                next_check_at,
                in_flight: false,
                after_auto_update,
            });
        }
    }

    pub(crate) fn due_cargo_hash_polls_at(&self, now: SystemTime) -> Vec<String> {
        self.cargo_hash_polls
            .iter()
            .filter(|poll| !poll.in_flight && poll.next_check_at <= now)
            .map(|poll| poll.repo_name.clone())
            .collect()
    }

    pub(crate) fn mark_cargo_hash_poll_in_flight(&mut self, repo_name: &str) {
        if let Some(poll) = self
            .cargo_hash_polls
            .iter_mut()
            .find(|poll| poll.repo_name == repo_name)
        {
            poll.in_flight = true;
        }
    }

    pub(crate) fn stop_cargo_hash_polling(&mut self, repo_name: &str) {
        self.cargo_hash_polls
            .retain(|poll| poll.repo_name != repo_name);
    }

    pub(crate) fn finish_cargo_hash_poll_attempt_at(
        &mut self,
        repo_name: &str,
        now: SystemTime,
    ) -> bool {
        if let Some(idx) = self
            .cargo_hash_polls
            .iter()
            .position(|poll| poll.repo_name == repo_name)
        {
            if now
                .duration_since(self.cargo_hash_polls[idx].started_at)
                .unwrap_or(Duration::ZERO)
                >= CARGO_HASH_POLL_TIMEOUT
            {
                self.cargo_hash_polls.remove(idx);
                true
            } else {
                self.cargo_hash_polls[idx].in_flight = false;
                self.cargo_hash_polls[idx].next_check_at = now + CARGO_HASH_POLL_INTERVAL;
                false
            }
        } else {
            false
        }
    }

    #[cfg(test)]
    pub(crate) fn expire_cargo_hash_polls_at(&mut self, now: SystemTime) -> Vec<String> {
        self.take_expired_cargo_hash_polls_at(now)
            .into_iter()
            .map(|poll| poll.repo_name)
            .collect()
    }

    pub(crate) fn take_expired_cargo_hash_polls_at(
        &mut self,
        now: SystemTime,
    ) -> Vec<ExpiredCargoHashPoll> {
        let expired: Vec<ExpiredCargoHashPoll> = self
            .cargo_hash_polls
            .iter()
            .filter(|poll| {
                now.duration_since(poll.started_at)
                    .unwrap_or(Duration::ZERO)
                    >= CARGO_HASH_POLL_TIMEOUT
            })
            .map(|poll| ExpiredCargoHashPoll {
                repo_name: poll.repo_name.clone(),
                after_auto_update: poll.after_auto_update,
            })
            .collect();
        for poll in &expired {
            self.stop_cargo_hash_polling(&poll.repo_name);
        }
        expired
    }

    pub(crate) fn active_cargo_hash_poll_count(&self) -> usize {
        self.cargo_hash_polls.len()
    }

    pub(crate) fn cargo_hash_poll_after_auto_update(&self, repo_name: &str) -> bool {
        self.cargo_hash_polls
            .iter()
            .find(|poll| poll.repo_name == repo_name)
            .is_some_and(|poll| poll.after_auto_update)
    }
}
