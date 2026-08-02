use crate::account::api_keys;

use super::{DomainError, DomainResult, PasteService, Principal};

impl PasteService {
    fn key_owner(&self, principal: &Principal) -> DomainResult<i64> {
        let owner = principal
            .user_id()
            .ok_or_else(|| DomainError::forbidden("User identity required"))?;
        if matches!(principal, Principal::ApiKey(key) if !key.has_scope("api_key:manage")) {
            return Err(DomainError::forbidden("Missing api_key:manage permission"));
        }
        Ok(owner)
    }

    pub async fn list_api_keys(
        &self,
        principal: &Principal,
    ) -> DomainResult<Vec<api_keys::ApiKey>> {
        api_keys::list_for_user(&self.storage, self.key_owner(principal)?).await
    }

    pub async fn create_api_key(
        &self,
        principal: &Principal,
        name: &str,
        scopes: &[String],
    ) -> DomainResult<(api_keys::ApiKey, String)> {
        let owner = self.key_owner(principal)?;
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 100 {
            return Err(DomainError::validation_code(
                "invalid_api_key_name",
                "Key name must contain 1 to 100 characters",
            ));
        }
        let scopes = api_keys::normalize_scopes(scopes)
            .map_err(|message| DomainError::validation_code("invalid_api_key_scopes", message))?;
        if let Principal::ApiKey(key) = principal {
            if scopes.iter().any(|scope| !key.has_scope(scope)) {
                return Err(DomainError::forbidden(
                    "A key can only grant scopes it holds",
                ));
            }
        } else if !principal.is_admin() && scopes.iter().any(|scope| scope.ends_with(":manage")) {
            return Err(DomainError::forbidden(
                "Only administrators can grant administrative scopes",
            ));
        }
        api_keys::create(&self.storage, Some(owner), name, &scopes).await
    }

    pub async fn set_api_key_enabled(
        &self,
        principal: &Principal,
        id: i64,
        enabled: bool,
    ) -> DomainResult<bool> {
        api_keys::set_enabled_for_user(&self.storage, id, self.key_owner(principal)?, enabled).await
    }

    pub async fn delete_api_key(&self, principal: &Principal, id: i64) -> DomainResult<bool> {
        api_keys::delete_for_user(&self.storage, id, self.key_owner(principal)?).await
    }
}
