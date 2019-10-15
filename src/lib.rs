pub mod configuration;
pub mod error;
pub mod handlers;
pub mod macros;
pub mod service;

use {
    error::{Error, ErrorKind, Result},
    hyper::{Body, Request},
    lazy_static::lazy_static,
    serde::{Deserialize, Serialize},
    slog::Logger,
    std::collections::HashMap,
};

pub mod log {
    use {
        lazy_static::lazy_static,
        slog::{o, Drain, LevelFilter, Logger},
        slog_async::Async,
        slog_term::{CompactFormat, TermDecorator},
    };

    lazy_static! {
        // The "base" logger that all crates should branch off of
        pub static ref BASE_LOG: Logger = {
            let decorator = TermDecorator::new().build();
            let drain = CompactFormat::new(decorator).build().fuse();
            let drain = Async::new(drain).build().fuse();
            let drain = LevelFilter::new(drain, slog::Level::Debug).fuse();
            let log = Logger::root(drain, o!());
            log
        };
    }
}

lazy_static! {
    pub static ref LOG: Logger = { log::BASE_LOG.new(slog::o!("mod" => "mpix")) };
}

pub struct Caps {
    inner: Option<HashMap<String, String>>,
}
impl Caps {
    pub fn empty() -> Self {
        Self { inner: None }
    }

    pub fn with(capture_map: HashMap<String, String>) -> Self {
        Self {
            inner: Some(capture_map),
        }
    }

    pub fn get<T: AsRef<str>>(&self, s: T) -> Result<String> {
        let s = s.as_ref();
        let val = self
            .inner
            .as_ref()
            .and_then(|cap_map| cap_map.get(s).map(String::from))
            .ok_or_else(|| {
                ErrorKind::MissingUriParam(format!("missing expected uri parameter '{}'", s))
            })?;
        Ok(val)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    name: String,
}
impl redis::FromRedisValue for User {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<User> {
        match *v {
            redis::Value::Data(ref bytes) => Ok(serde_json::from_slice(bytes)
                .map_err(|_| (redis::ErrorKind::TypeError, "Invalid user json bytes"))?),
            _ => Err((
                redis::ErrorKind::TypeError,
                "Response type not user compatible.",
            ))?,
        }
    }
}

pub struct Auth {
    pub user_token: String,
}

pub struct Context {
    request: Request<Body>,
    captures: Caps,
    auth: Option<Auth>,
    redis: redis::Client,
}
impl Context {
    fn redis() -> Result<redis::Client> {
        Ok(redis::Client::open(
            configuration::CONFIG.redis_url.as_ref(),
        )?)
    }

    pub fn with_req(r: Request<Body>) -> Result<Self> {
        Ok(Self {
            request: r,
            auth: None,
            captures: Caps::empty(),
            redis: Self::redis()?,
        })
    }

    pub fn new(r: Request<Body>, auth: Option<Auth>, caps: Caps) -> Result<Self> {
        Ok(Self {
            request: r,
            auth,
            captures: caps,
            redis: Self::redis()?,
        })
    }
}

#[derive(Debug, PartialEq)]
pub enum Environment {
    Local,
    Production,
}
impl std::str::FromStr for Environment {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(match s.trim().to_lowercase().as_ref() {
            "local" => Environment::Local,
            "production" => Environment::Production,
            s => Err(format!("Invalid env: {}", s))?,
        })
    }
}
