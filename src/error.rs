use std;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error {
    kind: Box<ErrorKind>,
}
impl Error {
    pub fn kind(&self) -> &ErrorKind {
        self.kind.as_ref()
    }

    pub fn from_kind(kind: ErrorKind) -> Self {
        Self {
            kind: Box::new(kind),
        }
    }

    pub fn is_does_not_exist(&self) -> bool {
        if let self::ErrorKind::DoesNotExist(_) = self.kind() {
            true
        } else {
            false
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use self::ErrorKind::*;
        match *self.kind() {
            S(ref s) => write!(f, "{}", s),
            Internal(ref s) => write!(f, "InternalError: {}", s),
            InvalidAuth(ref s) => write!(f, "InvalidAuth: {}", s),
            BadRequest(ref s) => write!(f, "BadRequest: {}", s),
            DoesNotExist(ref s) => write!(f, "DoesNotExist: {}", s),
            MissingUriParam(ref s) => write!(f, "MissingUriParam: {}", s),
            InvalidUriParam(ref s) => write!(f, "InvalidUriParam: {}", s),

            Hyper(ref e) => write!(f, "HyperError: {}", e),
            Http(ref e) => write!(f, "HttpError: {}", e),
            ParseAddr(ref e) => write!(f, "ParseAddr: {}", e),
            ParseInt(ref e) => write!(f, "ParseInt: {}", e),
            ParseBool(ref e) => write!(f, "ParseBool: {}", e),
            HeaderToStrError(ref e) => write!(f, "HeaderToStrError: {}", e),
            HeaderInvalidValue(ref e) => write!(f, "HeaderInvalidValue: {}", e),
            Redis(ref e) => write!(f, "RedisError: {}", e),
            Json(ref e) => write!(f, "JsonError: {}", e),
        }
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        "mpix error"
    }

    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        use self::ErrorKind::*;
        Some(match *self.kind() {
            Hyper(ref e) => e,
            Http(ref e) => e,
            ParseAddr(ref e) => e,
            ParseInt(ref e) => e,
            ParseBool(ref e) => e,
            Redis(ref e) => e,
            Json(ref e) => e,
            _ => return None,
        })
    }
}

#[derive(Debug)]
pub enum ErrorKind {
    S(String),
    Internal(String),
    InvalidAuth(String),
    BadRequest(String),
    DoesNotExist(String),
    MissingUriParam(String),
    InvalidUriParam(String),

    Hyper(hyper::Error),
    Http(http::Error),
    ParseAddr(std::net::AddrParseError),
    ParseInt(std::num::ParseIntError),
    ParseBool(std::str::ParseBoolError),
    HeaderToStrError(http::header::ToStrError),
    HeaderInvalidValue(http::header::InvalidHeaderValue),
    Redis(redis::RedisError),
    Json(serde_json::error::Error),
}

impl From<ErrorKind> for Error {
    fn from(k: ErrorKind) -> Error {
        Error { kind: Box::new(k) }
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Error {
        Error {
            kind: Box::new(ErrorKind::S(s.into())),
        }
    }
}

impl From<String> for Error {
    fn from(s: String) -> Error {
        Error {
            kind: Box::new(ErrorKind::S(s.into())),
        }
    }
}

impl From<hyper::Error> for Error {
    fn from(e: hyper::Error) -> Error {
        Error {
            kind: Box::new(ErrorKind::Hyper(e)),
        }
    }
}

impl From<http::Error> for Error {
    fn from(e: http::Error) -> Error {
        Error {
            kind: Box::new(ErrorKind::Http(e)),
        }
    }
}

impl From<std::net::AddrParseError> for Error {
    fn from(e: std::net::AddrParseError) -> Error {
        Error {
            kind: Box::new(ErrorKind::ParseAddr(e)),
        }
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(e: std::num::ParseIntError) -> Error {
        Error {
            kind: Box::new(ErrorKind::ParseInt(e)),
        }
    }
}

impl From<std::str::ParseBoolError> for Error {
    fn from(e: std::str::ParseBoolError) -> Error {
        Error {
            kind: Box::new(ErrorKind::ParseBool(e)),
        }
    }
}

impl From<http::header::ToStrError> for Error {
    fn from(e: http::header::ToStrError) -> Error {
        Error {
            kind: Box::new(ErrorKind::HeaderToStrError(e)),
        }
    }
}

impl From<http::header::InvalidHeaderValue> for Error {
    fn from(e: http::header::InvalidHeaderValue) -> Error {
        Error {
            kind: Box::new(ErrorKind::HeaderInvalidValue(e)),
        }
    }
}

impl From<redis::RedisError> for Error {
    fn from(e: redis::RedisError) -> Error {
        Error {
            kind: Box::new(ErrorKind::Redis(e)),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Error {
        Error {
            kind: Box::new(ErrorKind::Json(e)),
        }
    }
}
