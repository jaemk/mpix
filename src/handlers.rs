use crate::error::Result;
use crate::Context;
use {
    futures_util::{compat::Stream01CompatExt, TryStreamExt},
    hyper::{Body, Response, StatusCode},
};

lazy_static::lazy_static! {
    pub static ref LOG: slog::Logger = { crate::LOG.new(slog::o!("mod" => "handlers")) };
}

pub async fn hello(_ctx: Context) -> Result<Response<Body>> {
    Ok(Response::new(Body::from("hello")))
}

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

pub async fn not_found(_ctx: Context) -> Result<Response<Body>> {
    let r = Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("not found"))?;
    Ok(r)
}

pub async fn greet(ctx: Context) -> Result<Response<Body>> {
    let name = ctx.captures.get("name")?;
    Ok(Response::new(Body::from(format!("hello, {}!", name))))
}
