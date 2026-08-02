pub(crate) fn run_if_requested() -> Result<bool, String> {
    if std::env::args().nth(1).as_deref() != Some("openapi") {
        return Ok(false);
    }
    let document = crate::http::meta::openapi_document();
    println!(
        "{}",
        serde_json::to_string_pretty(&document).map_err(|error| error.to_string())?
    );
    Ok(true)
}
