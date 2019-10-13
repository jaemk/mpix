use {
    futures::future::{FutureExt, TryFutureExt},
    lazy_static::lazy_static,
    mpix::{log, service},
    std::net::SocketAddr,
};

lazy_static! {
    pub static ref LOG: slog::Logger = { log::BASE_LOG.new(slog::o!("mod" => "main")) };
}

fn main() {
    let host = if true { [0, 0, 0, 0] } else { [127, 0, 0, 1] };
    let port = 4000;
    let addr = SocketAddr::from((host, port));

    // `svr` is a `std::future` which we need to box and wrap to
    // make it compatible with hyper's futures01 runtime
    let svr = service::run(addr);
    let compat_svr = svr.unit_error().boxed().compat();

    hyper::rt::run(compat_svr);
}
