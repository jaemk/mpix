use crate::configuration::CONFIG;
use crate::error::Result;
use crate::Context;
use redis::PipelineCommands;
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
    let client = redis::Client::open("redis://localhost").unwrap();
    let conn = client.get_async_connection().compat().await?;
    let mut pipe = redis::pipe();
    pipe.incr("boop", 1)
        .ignore()
        .expire("boop", 5)
        .ignore()
        .get("boop");
    let (conn, (count,)): (_, (i32,)) = pipe.query_async(conn).compat().await?;
    let (_, cagain): (_, i32) = redis::cmd("GET")
        .arg("boop")
        .query_async(conn)
        .compat()
        .await?;

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

#[derive(Deserialize)]
struct CreateToken<'a> {
    description: &'a str,
}

pub async fn create(ctx: Context) -> Result<Response<Body>> {
    let auth = ctx
        .auth
        .ok_or_else(|| "in an authorized context without a token")?;
    let body = ctx.request.into_body().compat().try_concat().await?;
    let token_args: CreateToken = serde_json::from_slice(&body)?;
    let token = Token::new(token_args.description);
    let token_str = serde_json::to_string(&token)?;
    let conn = ctx.redis.get_async_connection().compat().await?;
    let key = format!("mpix.user:{}", auth.user_token);
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

pub async fn not_found(_ctx: Context) -> Result<Response<Body>> {
    let r = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("not found"))?;
    Ok(r)
}
