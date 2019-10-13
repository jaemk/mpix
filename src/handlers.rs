use crate::error::Result;
use crate::Context;
use {
    futures_util::{
        TryStreamExt,
        compat::{
            Stream01CompatExt,
            Future01CompatExt,
        },
    },
    hyper::{Body, Response, StatusCode},
};
use redis::PipelineCommands;

lazy_static::lazy_static! {
    pub static ref LOG: slog::Logger = { crate::LOG.new(slog::o!("mod" => "handlers")) };
}

pub mod test {
    use super::*;
    pub async fn echo_reverse(ctx: Context) -> Result<Response<Body>> {
        let bytes = ctx
            .request
            .into_body()
            .compat()
            .try_concat()
            .await?
            .to_vec();
        let s = String::from_utf8(bytes).map_err(|_| "Invalid utf8 string")?;
        let rev = s.chars().rev().collect::<String>();
        Ok(Response::new(Body::from(rev)))
    }

    pub async fn echo_upper(ctx: Context) -> Result<Response<Body>> {
        let resp_stream = ctx.request.into_body().compat().map_ok(|chunk| {
            chunk
                .iter()
                .map(|b| b.to_ascii_uppercase())
                .collect::<Vec<u8>>()
        });
        Ok(Response::new(Body::wrap_stream(resp_stream.compat())))
    }
}

pub async fn index(_ctx: Context) -> Result<Response<Body>> {
    let client = redis::Client::open("redis://localhost").unwrap();
    let conn = client.get_async_connection().compat().await?;
    let mut pipe = redis::pipe();
    pipe.incr("boop", 1).ignore()
        .expire("boop", 5).ignore()
        .get("boop");
    let (conn, (count,)): (_, (i32,)) = pipe.query_async(conn).compat().await?;
    let (_, cagain): (_, i32) = redis::cmd("GET").arg("boop").query_async(conn).compat().await?;
    Ok(Response::new(Body::from(format!("hello! {:?}", cagain))))
}

pub async fn not_found(_ctx: Context) -> Result<Response<Body>> {
    let r = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("not found"))?;
    Ok(r)
}
