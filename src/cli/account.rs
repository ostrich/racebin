use crate::account as accounts;
use crate::repository::Repository;
use std::fs;

fn password(arguments: &[String]) -> Result<String, String> {
    if let Some(index) = arguments
        .iter()
        .position(|value| value == "--password-file")
    {
        let path = arguments
            .get(index + 1)
            .ok_or("--password-file requires a path")?;
        return fs::read_to_string(path)
            .map(|password| password.trim_end_matches(['\r', '\n']).to_string())
            .map_err(|error| error.to_string());
    }
    rpassword::prompt_password("Password: ").map_err(|error| error.to_string())
}

fn option(arguments: &[String], name: &str) -> Option<String> {
    arguments
        .iter()
        .position(|value| value == name)
        .and_then(|index| arguments.get(index + 1))
        .cloned()
}

async fn user_id(repository: &Repository, username: &str) -> Result<i64, String> {
    sqlx::query_scalar("SELECT id FROM app_user WHERE username=$1")
        .bind(username)
        .fetch_optional(repository.pool())
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("account not found: {username}"))
}

pub(crate) async fn run_if_requested() -> Result<bool, String> {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments.get(1).map(String::as_str) != Some("account") {
        return Ok(false);
    }
    let data_dir = option(&arguments, "--data-dir")
        .or_else(|| std::env::var("RACEBIN_DATA_DIR").ok())
        .unwrap_or_else(|| "racebin_data".to_string());
    fs::create_dir_all(&data_dir).map_err(|error| error.to_string())?;
    let database_url = option(&arguments, "--database-url")
        .or_else(|| std::env::var("RACEBIN_DATABASE_URL").ok())
        .unwrap_or_else(|| format!("sqlite://{data_dir}/database.sqlite?mode=rwc"));
    let repository = Repository::open(&database_url, &data_dir).await?;
    repository.migrate().await?;
    let command = arguments.get(2).map(String::as_str).unwrap_or("help");
    match command {
        "create" => {
            let username = accounts::validate_username(arguments.get(3).ok_or(
                "usage: racebin account create USERNAME [--admin] [--password-file PATH]",
            )?)?;
            let role = if arguments.iter().any(|value| value == "--admin") {
                "admin"
            } else {
                "user"
            };
            sqlx::query(
                "INSERT INTO app_user(username,password_hash,role,enabled,force_password_change,created)
                 VALUES($1,$2,$3,1,0,$4)",
            )
            .bind(username)
            .bind(accounts::password_hash(&password(&arguments)?)?)
            .bind(role)
            .bind(accounts::now())
            .execute(repository.pool())
            .await
            .map_err(|error| error.to_string())?;
            println!("created {role} account {username}");
        }
        "list" => {
            for user in accounts::list_users(&repository).await? {
                println!(
                    "{}\t{}\t{}",
                    user.username,
                    user.role,
                    if user.enabled { "enabled" } else { "disabled" }
                );
            }
        }
        "password" => {
            let username = arguments
                .get(3)
                .ok_or("usage: racebin account password USERNAME [--password-file PATH]")?;
            let id = user_id(&repository, username).await?;
            accounts::set_password(&repository, id, &password(&arguments)?, false).await?;
            println!("password updated for {username}; existing sessions revoked");
        }
        "enable" | "disable" => {
            let username = arguments
                .get(3)
                .ok_or_else(|| format!("usage: racebin account {command} USERNAME"))?;
            let id = user_id(&repository, username).await?;
            accounts::set_enabled(&repository, id, command == "enable").await?;
            println!("{command}d account {username}");
        }
        "role" => {
            let username = arguments
                .get(3)
                .ok_or("usage: racebin account role USERNAME user|admin")?;
            let role = arguments.get(4).map(String::as_str).unwrap_or("");
            if !matches!(role, "user" | "admin") {
                return Err("role must be user or admin".to_string());
            }
            let id = user_id(&repository, username).await?;
            accounts::set_role(&repository, id, role == "admin").await?;
            println!("set {username} role to {role}");
        }
        _ => println!(
            "usage: racebin account <create|list|password|enable|disable|role> [arguments]\n\
             use --database-url URL to select the database"
        ),
    }
    Ok(true)
}
