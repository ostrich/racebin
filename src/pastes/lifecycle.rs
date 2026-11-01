//! Internal lifecycle decisions. Persistent values and HTTP representations stay unchanged.
use super::{DomainError, DomainResult, Paste, Principal};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CreationState {
    Pending,
    Complete,
}

impl CreationState {
    pub(super) fn for_attachments(count: usize) -> Self {
        if count == 0 {
            Self::Complete
        } else {
            Self::Pending
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Complete => "complete",
        }
    }
}

/// Only a new pending operation owns completion/rollback. A replay never does.
#[derive(Debug)]
pub enum CreationOwnership {
    Unkeyed,
    Idempotent { key: String, token: String },
}

#[derive(Debug)]
pub enum PasteCreation {
    Complete(Paste),
    Pending {
        paste: Paste,
        ownership: CreationOwnership,
    },
    Replayed(Paste),
}

impl PasteCreation {
    pub fn paste(&self) -> &Paste {
        match self {
            Self::Complete(paste) | Self::Replayed(paste) | Self::Pending { paste, .. } => paste,
        }
    }

    pub fn is_replay(&self) -> bool {
        matches!(self, Self::Replayed(_))
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum IdempotencyState {
    Pending,
    Completed,
}

impl IdempotencyState {
    pub(super) fn parse(value: &str) -> DomainResult<Self> {
        match value {
            "pending" => Ok(Self::Pending),
            "completed" => Ok(Self::Completed),
            _ => Err(DomainError::internal(
                "Invalid persisted creation operation state",
            )),
        }
    }
}

pub(super) const READ_CAPABILITY_SECONDS: i64 = 900;

/// An expiry belongs to the logical read, not to the time of its replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReadCapabilityExpiry(i64);

impl ReadCapabilityExpiry {
    pub(super) fn fresh(now: i64) -> Self {
        Self(now + READ_CAPABILITY_SECONDS)
    }
    pub(super) fn replay(expires_at: i64, now: i64) -> Option<Self> {
        (expires_at > now).then_some(Self(expires_at))
    }
    pub(super) fn timestamp(self) -> i64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReadTransition {
    Exempt,
    Counted { count: i64 },
    Final { count: i64 },
}

impl ReadTransition {
    pub(super) fn decide(principal: &Principal, paste: &Paste) -> Self {
        if principal.can_bypass_read_limit(paste.owner_id) {
            return Self::Exempt;
        }
        Self::counted(paste.read_count, paste.read_limit)
    }

    fn counted(current: i64, limit: Option<i64>) -> Self {
        let count = current + 1;
        if limit.is_some_and(|limit| count >= limit) {
            Self::Final { count }
        } else {
            Self::Counted { count }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation_and_persisted_operation_states_are_closed() {
        assert_eq!(CreationState::for_attachments(0), CreationState::Complete);
        assert_eq!(CreationState::for_attachments(1), CreationState::Pending);
        assert_eq!(
            IdempotencyState::parse("pending").unwrap(),
            IdempotencyState::Pending
        );
        assert_eq!(
            IdempotencyState::parse("completed").unwrap(),
            IdempotencyState::Completed
        );
        assert!(IdempotencyState::parse("unknown").is_err());
    }

    #[test]
    fn replay_cannot_extend_or_resurrect_a_read_capability() {
        let expiry = ReadCapabilityExpiry::fresh(100).timestamp();
        assert_eq!(
            ReadCapabilityExpiry::replay(expiry, expiry - 1)
                .unwrap()
                .timestamp(),
            expiry
        );
        assert_eq!(ReadCapabilityExpiry::replay(expiry, expiry), None);
    }

    #[test]
    fn only_the_final_counted_read_consumes_a_limited_paste() {
        assert_eq!(
            ReadTransition::counted(0, None),
            ReadTransition::Counted { count: 1 }
        );
        assert_eq!(
            ReadTransition::counted(0, Some(2)),
            ReadTransition::Counted { count: 1 }
        );
        assert_eq!(
            ReadTransition::counted(1, Some(2)),
            ReadTransition::Final { count: 2 }
        );
        assert_eq!(
            ReadTransition::counted(0, Some(1)),
            ReadTransition::Final { count: 1 }
        );
    }
}
