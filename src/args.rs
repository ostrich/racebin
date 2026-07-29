use clap::Parser;
use once_cell::sync::Lazy;
use std::net::IpAddr;

pub static ARGS: Lazy<Args> = Lazy::new(Args::parse);

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

    #[clap(long, env = "RACEBIN_PUBLIC_URL")]
    pub public_url: Option<url::Url>,

    #[clap(long, env = "RACEBIN_TITLE")]
    pub title: Option<String>,

    #[clap(short, long, env = "RACEBIN_NO_FILE_UPLOAD")]
    pub no_file_upload: bool,

    #[clap(long, env = "RACEBIN_MAX_FILE_SIZE_MB", default_value_t = 2048)]
    pub max_file_size_mb: usize,

    #[clap(long, env = "RACEBIN_QR")]
    pub qr: bool,

    #[clap(long, env = "RACEBIN_INSECURE_COOKIE")]
    pub insecure_cookie: bool,
}
