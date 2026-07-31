use clap::Parser;
use std::net::IpAddr;
use std::sync::LazyLock;

pub static ARGS: LazyLock<Args> = LazyLock::new(|| {
    #[cfg(test)]
    {
        Args::parse_from(["racebin"])
    }
    #[cfg(not(test))]
    {
        Args::parse()
    }
});

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    #[clap(short, long, env = "RACEBIN_PORT", default_value_t = 7042)]
    pub port: u16,

    #[clap(short, long, env = "RACEBIN_BIND", default_value_t = IpAddr::from([0, 0, 0, 0]))]
    pub bind: IpAddr,

    #[clap(short, long, env = "RACEBIN_THREADS", default_value_t = 2)]
    pub threads: u8,

    #[clap(long, env = "RACEBIN_DATA_DIR", default_value = "racebin_data")]
    pub data_dir: String,

    #[clap(long, env = "RACEBIN_DATABASE_URL")]
    pub database_url: Option<String>,

    #[clap(long, env = "RACEBIN_PUBLIC_URL")]
    pub public_url: Option<url::Url>,

    #[clap(long = "site-name", env = "RACEBIN_SITE_NAME")]
    pub site_name: Option<String>,

    #[clap(long = "plain-home", env = "RACEBIN_PLAIN_HOME")]
    pub plain_home: bool,

    #[clap(
        long = "attachments",
        env = "RACEBIN_ATTACHMENTS",
        default_value = "true",
        parse(try_from_str)
    )]
    pub attachments_enabled: bool,

    #[clap(
        long = "max-attachment-size-mb",
        env = "RACEBIN_MAX_ATTACHMENT_SIZE_MB",
        default_value_t = 2048
    )]
    pub max_attachment_size_mb: usize,

    #[clap(long = "qr-codes", env = "RACEBIN_QR_CODES")]
    pub qr_codes: bool,

    #[clap(long, env = "RACEBIN_INSECURE_COOKIE")]
    pub insecure_cookie: bool,
}

impl Args {
    pub fn effective_database_url(&self) -> String {
        self.database_url.clone().unwrap_or_else(|| {
            format!(
                "sqlite://{}?mode=rwc",
                std::path::Path::new(&self.data_dir)
                    .join("database.sqlite")
                    .to_string_lossy()
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Args;
    use clap::Parser;

    #[test]
    fn plain_home_is_opt_in() {
        assert!(!Args::parse_from(["racebin"]).plain_home);
        assert!(Args::parse_from(["racebin", "--plain-home"]).plain_home);
    }

    #[test]
    fn attachments_use_a_positive_boolean_option() {
        assert!(Args::parse_from(["racebin"]).attachments_enabled);
        assert!(Args::parse_from(["racebin", "--attachments", "true"]).attachments_enabled);
        assert!(!Args::parse_from(["racebin", "--attachments", "false"]).attachments_enabled);
        assert!(Args::try_parse_from(["racebin", "--disable-attachments"]).is_err());
    }
}
