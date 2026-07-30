use super::{Paste, PasteInput, Principal};

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
) -> Result<(), String> {
    if principal.can("paste:manage")
        || (principal.user_id() == paste.owner_id
            && (matches!(principal, Principal::Session(_)) || principal.can(scope)))
    {
        Ok(())
    } else {
        Err("You do not own this paste".into())
    }
}

pub(super) fn validate_input(input: &PasteInput, creating: bool) -> Result<(), String> {
    if input
        .title
        .as_deref()
        .is_some_and(|value| value.chars().count() > 200)
    {
        return Err("Title exceeds 200 characters".into());
    }
    if input
        .content_kind
        .as_deref()
        .is_some_and(|value| !matches!(value, "text" | "rich_text" | "redirect"))
    {
        return Err("Content kind must be text, rich_text, or redirect".into());
    }
    if input
        .visibility
        .as_deref()
        .is_some_and(|value| !matches!(value, "public" | "unlisted" | "private"))
    {
        return Err("Visibility must be public, unlisted, or private".into());
    }
    if input
        .read_limit
        .is_some_and(|value| value.is_some_and(|limit| limit <= 0))
    {
        return Err("Read limit must be positive or null".into());
    }
    if creating {
        validate_url(
            input.content_kind.as_deref().unwrap_or("text"),
            input.content.as_deref().unwrap_or(""),
        )?;
    }
    Ok(())
}

pub(super) fn validate_url(content_kind: &str, content: &str) -> Result<(), String> {
    if content_kind != "redirect" {
        return Ok(());
    }
    let parsed = url::Url::parse(content).map_err(|_| "URL content is invalid")?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Redirect pastes support only http and https".into());
    }
    Ok(())
}
