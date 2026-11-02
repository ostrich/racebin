use crate::database::{Database, DatabaseKind};
use crate::domain_error::{DomainError, DomainResult};
use crate::time::unix_timestamp;

mod administration;
pub mod api_keys;
mod identity;
mod invitations;
mod lifecycle;
mod throttling;

pub use administration::*;
pub use identity::*;
pub(super) use identity::{hash, random_token};
pub use invitations::*;
pub use throttling::*;

const ACTIVITY_REFRESH_SECONDS: i64 = 300;

fn activity_refresh_due(last_used_at: Option<i64>, now: i64) -> bool {
    last_used_at.is_none_or(|last| last < now.saturating_sub(ACTIVITY_REFRESH_SECONDS))
}

#[cfg(test)]
mod tests {
    use super::activity_refresh_due;

    #[test]
    fn activity_updates_preserve_the_five_minute_boundary() {
        assert!(activity_refresh_due(None, 1000));
        assert!(activity_refresh_due(Some(699), 1000));
        assert!(!activity_refresh_due(Some(700), 1000));
        assert!(!activity_refresh_due(Some(999), 1000));
        assert!(!activity_refresh_due(Some(1001), 1000));
    }
}
