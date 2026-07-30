use crate::repository::copy_database;

pub(crate) async fn run_if_requested() -> Result<bool, String> {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments.get(1).map(String::as_str) != Some("database")
        || arguments.get(2).map(String::as_str) != Some("copy")
    {
        return Ok(false);
    }
    let option = |name: &str| {
        arguments
            .iter()
            .position(|value| value == name)
            .and_then(|index| arguments.get(index + 1))
            .cloned()
    };
    let source = option("--from").ok_or("database copy requires --from URL")?;
    let destination = option("--to").ok_or("database copy requires --to URL")?;
    let data_dir = option("--data-dir")
        .or_else(|| std::env::var("RACEBIN_DATA_DIR").ok())
        .unwrap_or_else(|| "racebin_data".to_string());
    copy_database(&source, &destination, &data_dir).await?;
    println!("database copy completed and verified");
    Ok(true)
}
