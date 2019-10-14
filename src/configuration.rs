use crate::Environment;
use std::env;

lazy_static::lazy_static! {
    pub static ref LOG: slog::Logger = { crate::LOG.new(slog::o!("mod" => "configuration")) };
    pub static ref CONFIG: Config = Config::load();
}

pub struct Config {
    pub env: Environment,
    pub redis_url: String,
    pub auth_token: String,
}
impl Config {
    pub fn load() -> Self {
        Self {
            env: env::var("ENV")
                .expect("missing var: env")
                .parse::<Environment>()
                .expect("invalid env"),
            redis_url: env::var("REDIS_URL").expect("missing var: redis_url"),
            auth_token: env::var("AUTH_TOKEN").expect("missing var: auth_token"),
        }
    }
}
