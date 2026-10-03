#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    Restore,
    Tray,
    Exit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Main,
    Opening(u64),
    Visible(u64),
    Closing(u64, CloseReason),
    Hidden,
    Exiting,
}

pub struct Lifecycle {
    pub phase: Phase,
    next: u64,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self {
            phase: Phase::Main,
            next: 0,
        }
    }
}

impl Lifecycle {
    pub fn begin(&mut self) -> Option<u64> {
        if !matches!(self.phase, Phase::Main | Phase::Hidden) {
            return None;
        }
        self.next += 1;
        self.phase = Phase::Opening(self.next);
        Some(self.next)
    }

    pub fn token(&self) -> Option<u64> {
        match self.phase {
            Phase::Opening(id) | Phase::Visible(id) | Phase::Closing(id, _) => Some(id),
            _ => None,
        }
    }

    pub fn accepts_ready(&self, id: u64) -> bool {
        self.phase == Phase::Opening(id)
    }
    pub fn is_current(&self, id: u64) -> bool {
        matches!(self.phase, Phase::Opening(token) | Phase::Visible(token) if token == id)
    }
    pub fn needs_tray_recovery(&self) -> bool {
        matches!(
            self.phase,
            Phase::Hidden | Phase::Closing(_, CloseReason::Tray)
        )
    }
    pub fn begin_close(&mut self, reason: CloseReason) -> Option<(Phase, Option<u64>)> {
        if self.phase == Phase::Exiting {
            return None;
        }
        let previous = self.phase;
        let token = self.token();
        self.phase = token
            .map(|id| Phase::Closing(id, reason))
            .unwrap_or(Phase::Main);
        Some((previous, token))
    }
    pub fn abort_close(&mut self, previous: Phase, token: Option<u64>, reason: CloseReason) {
        let expected = token
            .map(|id| Phase::Closing(id, reason))
            .unwrap_or(Phase::Main);
        if self.phase == expected {
            self.phase = previous;
        }
    }
    pub fn finish_close(&mut self, id: u64) {
        if let Phase::Closing(token, reason) = self.phase {
            if token == id {
                self.phase = match reason {
                    CloseReason::Restore => Phase::Main,
                    CloseReason::Tray => Phase::Hidden,
                    CloseReason::Exit => Phase::Exiting,
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_cannot_be_overwritten_by_failed_restore() {
        let mut state = Lifecycle::default();
        let id = state.begin().unwrap();
        state.phase = Phase::Visible(id);
        let (previous, token) = state.begin_close(CloseReason::Restore).unwrap();
        state.phase = Phase::Exiting;
        state.abort_close(previous, token, CloseReason::Restore);
        assert_eq!(state.phase, Phase::Exiting);
        assert!(state.begin_close(CloseReason::Restore).is_none());
    }

    #[test]
    fn delayed_destroy_finishes_close_but_never_reverts_new_generation() {
        let mut state = Lifecycle::default();
        let id = state.begin().unwrap();
        let (previous, token) = state.begin_close(CloseReason::Restore).unwrap();
        state.finish_close(id);
        let next = state.begin().unwrap();
        state.abort_close(previous, token, CloseReason::Restore);
        state.finish_close(id);
        assert!(state.accepts_ready(next));
    }

    #[test]
    fn repeated_entry_has_one_generation() {
        let mut state = Lifecycle::default();
        let id = state.begin().unwrap();
        assert_eq!(state.begin(), None);
        state.phase = Phase::Visible(id);
        assert_eq!(state.begin(), None);
    }

    #[test]
    fn late_ready_and_destroy_cannot_affect_new_window() {
        let mut state = Lifecycle::default();
        let old = state.begin().unwrap();
        state.phase = Phase::Closing(old, CloseReason::Restore);
        assert!(!state.accepts_ready(old));
        state.finish_close(old);
        let new = state.begin().unwrap();
        assert!(!state.accepts_ready(old));
        state.finish_close(old);
        assert!(state.accepts_ready(new));
    }

    #[test]
    fn hide_and_exit_have_explicit_non_restore_outcomes() {
        for (reason, phase) in [
            (CloseReason::Tray, Phase::Hidden),
            (CloseReason::Exit, Phase::Exiting),
        ] {
            let mut state = Lifecycle::default();
            let id = state.begin().unwrap();
            state.phase = Phase::Closing(id, reason);
            assert!(!state.is_current(id));
            state.finish_close(id);
            assert_eq!(state.phase, phase);
            if phase == Phase::Exiting {
                assert_eq!(state.begin(), None);
            }
        }
    }

    #[test]
    fn tray_loss_is_recoverable_during_and_after_hide() {
        let mut state = Lifecycle::default();
        let token = state.begin().unwrap();
        assert!(!state.needs_tray_recovery());
        state.phase = Phase::Closing(token, CloseReason::Tray);
        assert!(state.needs_tray_recovery());
        state.finish_close(token);
        assert!(state.needs_tray_recovery());
        let next = state.begin().unwrap();
        assert!(!state.is_current(token));
        assert!(state.is_current(next));
    }
}
