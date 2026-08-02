use super::{DomainError, DomainResult, Paste, PasteInput, PasteQuery, Principal};

fn invalid_query(message: &'static str) -> DomainError {
    DomainError::validation_code("invalid_query", message)
}

pub(crate) fn validate_paste_query(query: &PasteQuery) -> DomainResult<()> {
    if query.folder_id.is_some() && query.unfiled.unwrap_or(false) {
        return Err(invalid_query("folder_id and unfiled cannot be combined"));
    }
    if (query.folder_id.is_some() || query.unfiled.unwrap_or(false)) && !query.mine.unwrap_or(false)
    {
        return Err(invalid_query("Folder filters require owner=me"));
    }
    if query.folder_id.is_some_and(|value| value < 1) {
        return Err(invalid_query("Folder ID must be positive"));
    }
    if query
        .visibility
        .as_deref()
        .is_some_and(|value| !matches!(value, "public" | "unlisted" | "private"))
    {
        return Err(invalid_query(
            "Visibility must be public, unlisted, or private",
        ));
    }
    if query
        .content_kind
        .as_deref()
        .is_some_and(|value| !matches!(value, "text" | "rich_text"))
    {
        return Err(invalid_query("Format must be text or rich_text"));
    }
    if query
        .expiration
        .as_deref()
        .is_some_and(|value| !matches!(value, "never" | "scheduled"))
    {
        return Err(invalid_query("Expiration must be never or scheduled"));
    }
    if query
        .read_limit
        .as_deref()
        .is_some_and(|value| !matches!(value, "unlimited" | "limited"))
    {
        return Err(invalid_query("Read limit must be unlimited or limited"));
    }
    if query
        .sort
        .as_deref()
        .is_some_and(|value| !matches!(value, "created" | "title" | "reads" | "expires" | "size"))
    {
        return Err(invalid_query("Unknown sort field"));
    }
    if query
        .direction
        .as_deref()
        .is_some_and(|value| !matches!(value, "asc" | "desc"))
    {
        return Err(invalid_query("Direction must be asc or desc"));
    }
    if matches!((query.created_after, query.created_before), (Some(after), Some(before)) if after > before)
    {
        return Err(invalid_query(
            "Created-after date must precede created-before date",
        ));
    }
    if matches!((query.min_reads, query.max_reads), (Some(minimum), Some(maximum)) if minimum > maximum)
    {
        return Err(invalid_query("Minimum reads cannot exceed maximum reads"));
    }
    if [
        query.min_reads,
        query.max_reads,
        query.min_size_bytes,
        query.max_size_bytes,
    ]
    .into_iter()
    .flatten()
    .any(|value| value < 0)
    {
        return Err(invalid_query("Read counts and sizes cannot be negative"));
    }
    if matches!((query.min_size_bytes, query.max_size_bytes), (Some(minimum), Some(maximum)) if minimum > maximum)
    {
        return Err(invalid_query("Minimum size cannot exceed maximum size"));
    }
    Ok(())
}

pub(super) fn can_read(principal: &Principal, paste: &Paste) -> bool {
    if paste.visibility != "private" {
        return true;
    }
    principal.can("paste:manage")
        || match principal {
            Principal::Session(session) => Some(session.user.id) == paste.owner_id,
            Principal::ApiKey(key) => key.user_id == paste.owner_id && key.has_scope("paste:read"),
            Principal::Anonymous => false,
        }
}

pub(super) fn authorize_owner(
    principal: &Principal,
    paste: &Paste,
    scope: &str,
) -> DomainResult<()> {
    if principal.can("paste:manage")
        || (principal.user_id() == paste.owner_id
            && (matches!(principal, Principal::Session(_)) || principal.can(scope)))
    {
        Ok(())
    } else {
        Err(DomainError::forbidden("You do not own this paste"))
    }
}

pub(super) fn validate_input(input: &PasteInput, now: i64) -> DomainResult<()> {
    if input
        .title
        .as_deref()
        .is_some_and(|value| value.chars().count() > 200)
    {
        return Err(DomainError::validation("Title exceeds 200 characters"));
    }
    if input
        .content_kind
        .as_deref()
        .is_some_and(|value| !matches!(value, "text" | "rich_text"))
    {
        return Err(DomainError::validation(
            "Content kind must be text or rich_text",
        ));
    }
    if input
        .visibility
        .as_deref()
        .is_some_and(|value| !matches!(value, "public" | "unlisted" | "private"))
    {
        return Err(DomainError::validation(
            "Visibility must be public, unlisted, or private",
        ));
    }
    if input
        .read_limit
        .is_some_and(|value| value.is_some_and(|limit| limit <= 0))
    {
        return Err(DomainError::validation(
            "Read limit must be positive or null",
        ));
    }
    if input
        .expires_at
        .is_some_and(|value| value.is_some_and(|expires_at| expires_at <= now))
    {
        return Err(DomainError::validation("Expiration must be in the future"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain_error::ErrorKind;

    #[test]
    fn paste_query_failures_remain_typed_domain_errors() {
        let error = validate_paste_query(&PasteQuery {
            visibility: Some("secret".into()),
            ..PasteQuery::default()
        })
        .unwrap_err();

        assert_eq!(error.kind, ErrorKind::Validation);
        assert_eq!(error.code, "invalid_query");
        assert_eq!(
            error.message,
            "Visibility must be public, unlisted, or private"
        );
    }
}
