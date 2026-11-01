use super::{DomainError, DomainResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AccountRole {
    User,
    Admin,
    Owner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AccountState {
    Active(AccountRole),
    Disabled(AccountRole),
}

impl AccountState {
    pub(super) fn stored(role: &str, enabled: bool, owner: bool) -> DomainResult<Self> {
        let role = match (role, owner) {
            ("admin", true) => AccountRole::Owner,
            ("admin", false) => AccountRole::Admin,
            ("user", false) => AccountRole::User,
            _ => return Err(DomainError::internal("Invalid persisted account role")),
        };
        Ok(if enabled {
            Self::Active(role)
        } else {
            Self::Disabled(role)
        })
    }

    fn role(self) -> AccountRole {
        match self {
            Self::Active(role) | Self::Disabled(role) => role,
        }
    }

    pub(super) fn enabled(self) -> bool {
        matches!(self, Self::Active(_))
    }
    pub(super) fn admin(self) -> bool {
        self.role() != AccountRole::User
    }
    pub(super) fn enabled_admin(self) -> bool {
        self.enabled() && self.admin()
    }

    /// Validate before issuing SQL; authorization of the caller remains at the service boundary.
    pub(super) fn update(self, enabled: Option<bool>, admin: Option<bool>) -> DomainResult<Self> {
        if self.role() == AccountRole::Owner && (enabled == Some(false) || admin == Some(false)) {
            return Err(DomainError::validation_code(
                "owner_protected",
                "The owner cannot be disabled or demoted",
            ));
        }
        let role = match (self.role(), admin) {
            (AccountRole::Owner, _) => AccountRole::Owner,
            (_, Some(true)) => AccountRole::Admin,
            (_, Some(false)) => AccountRole::User,
            (role, None) => role,
        };
        Ok(if enabled.unwrap_or(self.enabled()) {
            Self::Active(role)
        } else {
            Self::Disabled(role)
        })
    }

    pub(super) fn require_successor(self, enabled_admins: i64) -> DomainResult<()> {
        if enabled_admins <= 1 {
            return Err(DomainError::validation_code(
                "last_administrator",
                if !self.enabled() {
                    "The last enabled administrator cannot be disabled"
                } else {
                    "The last administrator cannot be demoted"
                },
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_and_administrator_transitions_are_explicit() {
        let owner = AccountState::Active(AccountRole::Owner);
        assert!(owner.update(Some(false), None).is_err());
        assert!(owner.update(None, Some(false)).is_err());
        assert_eq!(owner.update(None, None).unwrap(), owner);
        let admin = AccountState::Active(AccountRole::Admin);
        let disabled = admin.update(Some(false), None).unwrap();
        assert_eq!(disabled, AccountState::Disabled(AccountRole::Admin));
        assert!(disabled.require_successor(1).is_err());
        assert!(disabled.require_successor(2).is_ok());
        assert!(!admin.update(None, Some(false)).unwrap().admin());
        assert!(AccountState::stored("unexpected", true, false).is_err());
    }
}
