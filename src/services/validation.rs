use super::{Paste, PasteInput, Principal};

pub(super) fn can_read(principal: &Principal, paste: &Paste) -> bool {
    if paste.access != "owner" {
        return true;
    }
    principal.can("paste:admin")
        || match principal {
            Principal::User(session) => Some(session.user.id) == paste.owner_user_id,
            Principal::Key(key) => {
                key.user_id == paste.owner_user_id && key.has_scope("paste:read")
            }
            Principal::Anonymous => false,
        }
}

pub(super) fn authorize_owner(
    principal: &Principal,
    paste: &Paste,
    scope: &str,
) -> Result<(), String> {
    if principal.can("paste:admin")
        || (principal.user_id() == paste.owner_user_id
            && (matches!(principal, Principal::User(_)) || principal.can(scope)))
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
        .kind
        .as_deref()
        .is_some_and(|value| !matches!(value, "text" | "url"))
    {
        return Err("Kind must be text or url".into());
    }
    if input
        .access
        .as_deref()
        .is_some_and(|value| !matches!(value, "public" | "unlisted" | "owner"))
    {
        return Err("Access must be public, unlisted, or owner".into());
    }
    if input.burn_after_reads.is_some_and(|value| value < 0) {
        return Err("Burn count cannot be negative".into());
    }
    if creating {
        validate_url(
            input.kind.as_deref().unwrap_or("text"),
            input.content.as_deref().unwrap_or(""),
        )?;
    }
    Ok(())
}

pub(super) fn validate_url(kind: &str, content: &str) -> Result<(), String> {
    if kind != "url" {
        return Ok(());
    }
    let parsed = url::Url::parse(content).map_err(|_| "URL content is invalid")?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("URL pastes support only http and https".into());
    }
    Ok(())
}
