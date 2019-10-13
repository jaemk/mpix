pub mod error;
pub mod handlers;
pub mod macros;
pub mod service;

use {
    error::{ErrorKind, Result},
    hyper::{Body, Request},
    lazy_static::lazy_static,
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

pub struct Context {
    request: Request<Body>,
    captures: Caps,
}
