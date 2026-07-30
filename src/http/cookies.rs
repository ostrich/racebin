use crate::account::SESSION_COOKIE;
use crate::args::ARGS;
use actix_web::cookie::{Cookie, SameSite};

pub(super) fn session_cookie(token: String, remember: bool) -> Cookie<'static> {
    let mut builder = Cookie::build(SESSION_COOKIE, token)
        .path("/")
        .http_only(true)
        .secure(!ARGS.insecure_cookie)
        .same_site(SameSite::Lax);
    if remember {
        builder = builder.max_age(actix_web::cookie::time::Duration::days(30));
    }
    builder.finish()
}
