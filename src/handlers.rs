use crate::error::Result;
use crate::Context;
use {
    futures_util::{
        compat::{Future01CompatExt, Stream01CompatExt},
        TryStreamExt,
    },
    hyper::{Body, Response, StatusCode},
    serde::{Deserialize, Serialize},
};

lazy_static::lazy_static! {
    pub static ref LOG: slog::Logger = { crate::LOG.new(slog::o!("mod" => "handlers")) };
}

static INDEX: &'static str = r##"
<html>
    <head>
        <link rel="shortcut icon" href="/p/favicon?v=1" type="image/png">
    </head>
    hello! <img src="/p/163e71e1a222461fac0a139dddacf1d5"/>
</html>
"##;

pub async fn index(_ctx: Context) -> Result<Response<Body>> {
    let resp = Response::builder()
        .header("content-type", "text/html")
        .body(Body::from(INDEX))?;
    Ok(resp)
}

#[derive(Serialize, Deserialize)]
struct Token {
    token: String,
    description: String,
    created: chrono::DateTime<chrono::Local>,
}
impl Token {
    fn new<T: AsRef<str>>(description: T) -> Self {
        Self {
            token: uuid::Uuid::new_v4()
                .to_simple()
                .encode_lower(&mut uuid::Uuid::encode_buffer())
                .to_string(),
            description: description.as_ref().to_string(),
            created: chrono::Local::now(),
        }
    }
}
impl redis::FromRedisValue for Token {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<Token> {
        match *v {
            redis::Value::Data(ref bytes) => Ok(serde_json::from_slice(bytes)
                .map_err(|_| (redis::ErrorKind::TypeError, "Invalid token json bytes"))?),
            _ => Err((
                redis::ErrorKind::TypeError,
                "Response type not token compatible.",
            ))?,
        }
    }
}

#[derive(Deserialize)]
struct CreateToken<'a> {
    description: &'a str,
}

pub async fn create(ctx: Context) -> Result<Response<Body>> {
    let auth = ctx
        .auth
        .ok_or_else(|| "in an authorized context without a token")?;
    let body = ctx.request.into_body().compat().try_concat().await?;
    let token_args: CreateToken =
        serde_json::from_slice(&body).map_err(|e| format!("Invalid create token input: {}", e))?;
    let token = Token::new(token_args.description);
    let token_str = serde_json::to_string(&token)?;
    let conn = ctx.redis.get_async_connection().compat().await?;
    let key = format!("mpix.user_tokens:{}", auth.user_token);
    let _: (_, ()) = redis::cmd("HSET")
        .arg(key)
        .arg(token.token)
        .arg(&token_str)
        .query_async(conn)
        .compat()
        .await?;

    let r = Response::builder()
        .header("content-type", "application/json")
        .body(Body::from(token_str))?;
    Ok(r)
}

#[derive(Serialize, Deserialize)]
struct TokenData {
    created: chrono::DateTime<chrono::Local>,
}
impl TokenData {
    fn new() -> Self {
        Self {
            created: chrono::Local::now(),
        }
    }
}
impl redis::FromRedisValue for TokenData {
    fn from_redis_value(v: &redis::Value) -> redis::RedisResult<TokenData> {
        match *v {
            redis::Value::Data(ref bytes) => Ok(serde_json::from_slice(bytes)
                .map_err(|_| (redis::ErrorKind::TypeError, "Invalid token json bytes"))?),
            _ => Err((
                redis::ErrorKind::TypeError,
                "Response type not token compatible.",
            ))?,
        }
    }
}

pub async fn track(ctx: Context) -> Result<Response<Body>> {
    lazy_static::lazy_static! {
        static ref PIXEL: Vec<u8> = base64::decode(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII="
        ).expect("pixel is invalid base64");
    }

    let token = ctx.captures.get("token")?;
    let data = TokenData::new();
    let data_str = serde_json::to_string(&data)?;
    let list_key = format!("mpix.token:{}", token);
    let conn = ctx.redis.get_async_connection().compat().await?;
    let mut pipe = redis::Pipeline::new();
    pipe.atomic()
        .atomic()
        .cmd("LPUSH")
        .arg(&list_key)
        .arg(data_str)
        .ignore()
        .cmd("LTRIM")
        .arg(&list_key)
        .arg(0)
        .arg(200)
        .ignore()
        .cmd("LLEN")
        .arg(&list_key);
    let (_, (count,)): (_, (usize,)) = pipe.query_async(conn).compat().await?;

    slog::debug!(LOG, "tracked token"; "token" => token, "count" => count);
    let r = Response::builder()
        .header("content-type", "image/png")
        .body(Body::from(PIXEL.as_slice()))?;
    Ok(r)
}

pub async fn tracking_stats(ctx: Context) -> Result<Response<Body>> {
    let auth = ctx
        .auth
        .ok_or_else(|| "in an authorized context without a token")?;
    let conn = ctx.redis.get_async_connection().compat().await?;
    match ctx.captures.get("token").ok() {
        Some(token) => {
            #[derive(Serialize)]
            struct ReturnData {
                events: Vec<TokenData>,
            }

            let key = format!("mpix.token:{}", token);
            let (_, token_data): (_, Option<Vec<TokenData>>) = redis::cmd("LRANGE")
                .arg(key)
                .arg(0)
                .arg(-1)
                .query_async(conn)
                .compat()
                .await?;
            let resp = ReturnData {
                events: token_data.unwrap_or_else(|| vec![]),
            };
            Ok(Response::new(Body::from(serde_json::to_string(&resp)?)))
        }
        None => {
            #[derive(Serialize)]
            struct ReturnData {
                tokens: Vec<Token>,
            }

            let key = format!("mpix.user_tokens:{}", auth.user_token);
            let (_conn, tokens): (_, Option<Vec<Token>>) = redis::cmd("HVALS")
                .arg(key)
                .query_async(conn)
                .compat()
                .await?;
            let resp = ReturnData {
                tokens: tokens.unwrap_or_else(|| vec![]),
            };
            Ok(Response::new(Body::from(serde_json::to_string(&resp)?)))
        }
    }
}

#[derive(Serialize)]
struct Status<'a, 'b> {
    status: &'a str,
    hash: &'b str,
}

pub async fn status(_ctx: Context) -> Result<Response<Body>> {
    let st = Status {
        status: "ok",
        hash: include_str!("../commit_hash.txt").trim(),
    };
    let status = serde_json::to_string(&st)?;
    Ok(Response::new(Body::from(status)))
}

pub async fn not_found(_ctx: Context) -> Result<Response<Body>> {
    let r = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("not found"))?;
    Ok(r)
}
