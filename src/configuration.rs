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
        let redis_host = env::var("REDIS_HOST").expect("missing var: redis_host");
        let redis_pass= env::var("REDIS_PASSWORD").expect("missing var: redis_password");
        let redis_url = format!("redis://:{}@{}", redis_pass, redis_host);
        Self {
            env: env::var("ENV")
                .expect("missing var: env")
                .parse::<Environment>()
                .expect("invalid env"),
            redis_url: redis_url,
            auth_token: env::var("AUTH_TOKEN").expect("missing var: auth_token"),
        }
    }
}
