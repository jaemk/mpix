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
    std::{collections::HashSet, io::Write, net::SocketAddr},
};

use crate::configuration::CONFIG;
use crate::error::{Error, ErrorKind, Result};
use crate::{handlers, Auth};
use crate::{router, User};

lazy_static::lazy_static! {
    pub static ref LOG: slog::Logger = { crate::LOG.new(slog::o!("mod" => "service")) };
}

async fn is_valid_auth(auth_token: String) -> Result<Auth> {
    slog::debug!(LOG, "checking auth");
    let conn = redis::Client::open(CONFIG.redis_url.as_ref())?
        .get_async_connection()
        .compat()
        .await?;
    let (_, opt): (_, Option<User>) = redis::cmd("HGET")
        .arg("mpix.users")
        .arg(&auth_token)
        .query_async(conn)
        .compat()
        .await
        .unwrap();
    slog::debug!(LOG, "authorized user";
                 "user" => format!("{:?}", opt));
    if let Some(_) = opt {
        Ok(Auth {
            user_token: auth_token,
        })
    } else {
        Err(ErrorKind::InvalidAuth("missing auth token".into()))?
    }
}

/// Require an auth bearer token on requests
async fn ensure_auth(
    req: Request<Body>,
) -> Result<(Request<Body>, Option<Auth>, Option<Response<Body>>)> {
    lazy_static::lazy_static! {
        static ref ALLOWED: HashSet<&'static str> = maplit::hashset!{"", "/status"};
    };

    let path = req.uri().path().trim_end_matches("/");
    if ALLOWED.contains(path) || path.starts_with("/p/") {
        return Ok((req, None, None));
    }

    let maybe_auth = req
        .headers()
        .get("x-mpix-auth")
        .ok_or_else(|| Error::from("missing auth header"))
        .and_then(|hv| Ok(hv.to_str()?.to_string()))
        .ok();
    if let Some(auth_token) = maybe_auth {
        if let Some(auth) = is_valid_auth(auth_token).await.ok() {
            return Ok((req, Some(auth), None));
        }
    }

    let resp = Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .body(Body::from("unauthorized"))?;
    Ok((req, None, Some(resp)))
}

/// gzip response content if the request accepts gzip
async fn gzip_response(headers: HeaderMap, mut resp: Response<Body>) -> Result<Response<Body>> {
    if let Some(accept) = headers.get("accept-encoding") {
        if accept.to_str()?.contains("gzip") {
            resp.headers_mut()
                .insert("content-encoding", HeaderValue::from_str("gzip")?);

            // split so we can modify the body
            let (parts, bod) = resp.into_parts();

            let mut e = GzEncoder::new(Vec::new(), Compression::default());
            // `bod` is a futures01 stream that needs to be made std::future compatible
            let bytes = bod.compat().try_concat().await?;
            let bytes_size = bytes.len();
            e.write_all(bytes.as_ref())
                .map_err(|e| format!("error writing bytes to gzip encoder {:?}", e))?;
            let res = e
                .finish()
                .map_err(|e| format!("error finishing gzip {:?}", e))?;
            let res_size = res.len();
            let new_bod = Body::from(res);

            let resp = Response::from_parts(parts, new_bod);
            slog::debug!(
                LOG, "gzipped";
                "original_size" => bytes_size, "gzipped_size" => res_size
            );
            return Ok(resp);
        }
    }
    Ok(resp)
}

async fn route(
    req: Request<Body>,
    auth: Option<Auth>,
    method: Method,
    uri: String,
) -> Result<Response<Body>> {
    router!(
         req, auth, method, uri.trim_end_matches("/"),
         [Method::GET, r"^/p/(?P<token>[a-zA-Z0-9-_]+)$", {"token"}] -> handlers::track,
         [Method::GET, r"^/status$", {}] -> handlers::status,
         [Method::GET, r"^$", {}] -> handlers::index,
         [Method::POST, r"^/create$", {}] -> handlers::create,
         [Method::GET, r"^/stat$", {}] -> handlers::tracking_stats,
         [Method::GET, r"^/stat/(?P<token>[a-zA-Z0-9-_]+)$", {"token"}] -> handlers::tracking_stats,
         _ -> handlers::not_found,
    );
}

/// Pipe an incoming request through pre-processing, routing, and post-processing hooks
async fn process(req: Request<Body>) -> Result<Response<Body>> {
    let headers = req.headers().clone();

    // before
    let (req, auth, resp) = ensure_auth(req).await?;
    if let Some(resp) = resp {
        return Ok(resp);
    }

    // route
    let method = req.method().clone();
    let uri = req.uri().path().to_string();
    let resp = route(req, auth, method, uri).await?;

    // after
    let resp = gzip_response(headers, resp).await?;
    Ok(resp)
}

async fn serve(req: Request<Body>) -> Result<Response<Body>> {
    // capture incoming info for logs
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let start = std::time::Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();

    let response = match process(req).await {
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

/// Build a server future that can be passed to a runtime
pub async fn run(addr: SocketAddr) {
    slog::info!(LOG, "Listening"; "host" => format!("http://{}", addr));

    let server_future = Server::bind(&addr).serve(|| {
        service_fn(|req| {
            // `serve` returns a `std::future` so we need to box and
            // wrap it to make it futures01 compatible before handing
            // it over to hyper
            serve(req).boxed().compat()
        })
    });

    // and now `server_future` is a futures01 future that we need to
    // make `std::futures` compatible so we can `.await` it
    if let Err(e) = server_future.compat().await {
        slog::error!(LOG, "server error"; "error" => format!("{}", e));
    }
}
