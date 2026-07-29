use clap::Parser;
use lazy_static::lazy_static;
use serde::Serialize;
use std::convert::Infallible;
use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;

lazy_static! {
    pub static ref ARGS: Args = Args::parse();
}

#[derive(Parser, Debug, Clone, Serialize)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    #[clap(short, long, env = "RACEBIN_PORT", default_value_t = 8080)]
    pub port: u16,

    #[clap(short, long, env = "RACEBIN_BIND", default_value_t = IpAddr::from([0, 0, 0, 0]))]
    pub bind: IpAddr,

    #[clap(short, long, env = "RACEBIN_THREADS", default_value_t = 1)]
    pub threads: u8,

    #[clap(long, env = "RACEBIN_DATA_DIR", default_value = "racebin_data")]
    pub data_dir: String,

    #[clap(long, env = "RACEBIN_PUBLIC_PATH")]
    pub public_path: Option<PublicUrl>,

    #[clap(long, env = "RACEBIN_TITLE")]
    pub title: Option<String>,

    #[clap(long, env = "RACEBIN_DEFAULT_EXPIRY", default_value = "never")]
    pub default_expiry: String,

    #[clap(short, long, env = "RACEBIN_NO_FILE_UPLOAD")]
    pub no_file_upload: bool,

    #[clap(
        long,
        env = "RACEBIN_MAX_FILE_SIZE_MB",
        alias = "max-file-size-unencrypted-mb",
        default_value_t = 2048
    )]
    pub max_file_size_unencrypted_mb: usize,

    #[clap(long, env = "RACEBIN_QR")]
    pub qr: bool,
}

impl Args {
    pub fn public_path_as_str(&self) -> String {
        self.public_path
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicUrl(pub String);

impl fmt::Display for PublicUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl FromStr for PublicUrl {
    type Err = Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.trim_end_matches('/').to_owned()))
    }
}
