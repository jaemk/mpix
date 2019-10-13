use {
    flate2::{write::GzEncoder, Compression},
    futures::{
        compat::Future01CompatExt,
        future::{FutureExt, TryFutureExt},
    },
    futures_util::{compat::Stream01CompatExt, TryStreamExt},
    hyper::{
        header::{HeaderMap, HeaderValue},
        service::service_fn,
        Body, Method, Request, Response, Server, StatusCode,
    },
    std::{io::Write, net::SocketAddr},
};

use crate::error::Result;
use crate::handlers;
use crate::router;

lazy_static::lazy_static! {
    pub static ref LOG: slog::Logger = { crate::LOG.new(slog::o!("mod" => "service")) };
}

async fn extract_auth(req: Request<Body>) -> Result<(Request<Body>, Option<Response<Body>>)> {
    if let Some(auth) = req.headers().get("authorization") {
        if auth == "Bearer 123" {
            return Ok((req, None));
        }
    }
    let resp = Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(Body::from("unauthorized"))?;
    Ok((req, Some(resp)))
}

async fn gzip_response(headers: HeaderMap, mut resp: Response<Body>) -> Result<Response<Body>> {
    if let Some(accept) = headers.get("accept-encoding") {
        if accept.to_str()?.contains("gzip") {
            resp.headers_mut()
                .insert("content-encoding", HeaderValue::from_str("gzip")?);
            let (parts, bod) = resp.into_parts();

            let mut e = GzEncoder::new(Vec::new(), Compression::default());
            let bytes = bod.compat().try_concat().await?;
            let ch = bytes.as_ref();
            e.write_all(ch)
                .map_err(|e| format!("error writing bytes to gzip encoder {:?}", e))?;
            let res = e
                .finish()
                .map_err(|e| format!("error finishing gzip {:?}", e))?;
            let new_bod = Body::from(res);
            let resp = Response::from_parts(parts, new_bod);
            return Ok(resp);
        }
    }
    Ok(resp)
}

async fn route(req: Request<Body>, method: Method, uri: String) -> Result<Response<Body>> {
    router!(
         req, method, uri.trim_end_matches("/"),
         [Method::GET, r"^$", {}] -> handlers::hello,
         [Method::POST, r"^/echo/upper$", {}] -> handlers::echo_upper,
         [Method::POST, r"^/echo/reverse$", {}] -> handlers::echo_reverse,
         [Method::GET, r"^/greet/(?P<name>[\w]+)$", {"named"}] -> handlers::greet,
         [Method::GET, r"^/greet$", {}] -> handlers::greet,
         _ -> handlers::not_found,
    );
}

async fn pipe(req: Request<Body>) -> Result<Response<Body>> {
    let headers = req.headers().clone();

    // before
    let (req, resp) = extract_auth(req).await?;
    if let Some(resp) = resp {
        return Ok(resp);
    }

    // route
    let method = req.method().clone();
    let uri = req.uri().path().to_string();
    let resp = route(req, method, uri).await?;

    // after
    let resp = gzip_response(headers, resp).await?;
    Ok(resp)
}

async fn serve(req: Request<Body>) -> Result<Response<Body>> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let start = std::time::Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let response = match pipe(req).await {
        Ok(resp) => resp,
        Err(err) => {
            slog::error!(LOG, "handler error";
                         "error" => format!("{}", err));
            Response::builder()
                .status(500)
                .body("server error".into())?
        }
    };
    let status = response.status();
    let elap = start.elapsed();
    let elap_ms = (elap.as_secs_f32() * 1_000.) + (elap.subsec_nanos() as f32 / 1_000_000.);
    slog::info!(LOG, "request";
                "method" => method.as_str(),
                "status" => status.as_u16(),
                "uri" => uri.path(),
                "timestamp" => now,
                "elapsed_ms" => elap_ms);
    Ok(response)
}

pub async fn run(addr: SocketAddr) {
    slog::info!(LOG, "Listening";
                "host" => format!("http://{}", addr));

    let server_future = Server::bind(&addr).serve(|| service_fn(|req| serve(req).boxed().compat()));

    if let Err(e) = server_future.compat().await {
        slog::error!(LOG, "server error";
                     "error" => format!("{}", e));
    }
}
