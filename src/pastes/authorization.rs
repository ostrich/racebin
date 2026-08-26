use crate::accounts::{self as accounts, api_keys};
use crate::time::unix_timestamp;

#[derive(Clone, Debug)]
pub enum Principal {
    Anonymous,
    Session(accounts::SessionUser),
    ApiKey(api_keys::ApiKey),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Permission {
    ManagePastes,
    ManageUsers,
    ManageInvitations,
    ManageApiKeys,
    ManageAdministrators,
    ConfigureInstance,
    TransferOwnership,
    ReadAuditLog,
}

impl Permission {
    pub fn id(self) -> &'static str {
        match self {
            Self::ManagePastes => "paste:manage",
            Self::ManageUsers => "user:manage",
            Self::ManageInvitations => "invitation:manage",
            Self::ManageApiKeys => "api_key:manage",
            Self::ManageAdministrators => "administrator:manage",
            Self::ConfigureInstance => "instance:configure",
            Self::TransferOwnership => "ownership:transfer",
            Self::ReadAuditLog => "audit:read",
        }
    }

    fn owner_only(self) -> bool {
        matches!(
            self,
            Self::ManageAdministrators
                | Self::ConfigureInstance
                | Self::TransferOwnership
                | Self::ReadAuditLog
        )
    }
}

pub const ADMIN_PERMISSIONS: &[Permission] = &[
    Permission::ManagePastes,
    Permission::ManageUsers,
    Permission::ManageInvitations,
    Permission::ManageApiKeys,
];

pub const OWNER_PERMISSIONS: &[Permission] = &[
    Permission::ManageAdministrators,
    Permission::ConfigureInstance,
    Permission::TransferOwnership,
    Permission::ReadAuditLog,
];

impl Principal {
    pub fn user_id(&self) -> Option<i64> {
        match self {
            Self::Session(session) => Some(session.user.id),
            Self::ApiKey(key) => key.user_id,
            Self::Anonymous => None,
        }
    }

    pub fn is_admin(&self) -> bool {
        matches!(self, Self::Session(session) if session.user.is_admin())
    }

    pub fn is_owner(&self) -> bool {
        matches!(self, Self::Session(session) if session.user.is_owner())
    }

    pub fn allows(&self, permission: Permission) -> bool {
        match self {
            Self::Session(session) => {
                if permission.owner_only() {
                    session.user.is_owner()
                } else {
                    session.user.is_admin()
                }
            }
            Self::ApiKey(key) => !permission.owner_only() && key.has_scope(permission.id()),
            Self::Anonymous => false,
        }
    }

    pub fn permissions(&self) -> Vec<&'static str> {
        ADMIN_PERMISSIONS
            .iter()
            .chain(OWNER_PERMISSIONS)
            .copied()
            .filter(|permission| self.allows(*permission))
            .map(Permission::id)
            .collect()
    }

    pub fn has_recent_authentication(&self) -> bool {
        matches!(self, Self::Session(session) if session.reauthenticated_at
            .is_some_and(|timestamp| timestamp >= unix_timestamp() - accounts::RECENT_AUTHENTICATION_SECONDS))
    }

    pub(super) fn is_session_owner(&self, owner_id: Option<i64>) -> bool {
        matches!(self, Self::Session(session) if Some(session.user.id) == owner_id)
    }

    pub fn can(&self, scope: &str) -> bool {
        let permission = match scope {
            "paste:manage" => Some(Permission::ManagePastes),
            "user:manage" => Some(Permission::ManageUsers),
            "invitation:manage" => Some(Permission::ManageInvitations),
            "api_key:manage" => Some(Permission::ManageApiKeys),
            _ => None,
        };
        permission.map_or_else(
            || self.is_admin() || matches!(self, Self::ApiKey(key) if key.has_scope(scope)),
            |permission| self.allows(permission),
        )
    }
}
