lazy_static::lazy_static! {
    pub static ref LOG: slog::Logger = { crate::LOG.new(slog::o!("mod" => "macros")) };
}

#[macro_export]
macro_rules! router {
    (
        // incoming request info
        $request:expr, $method:expr, $uri:expr,

        // a set of cases to match the incoming request info
        // note: the last statement must be a catch all, `_ -> func`
        // ex.
        // ```
        // [Method::GET, "^/status$", {}] -> handlers::status,
        // [Method::POST, r"^/give/(?P<name>[\w]+)/(?P<number>[0-9]+)/dollars$", {"name", "number"}] -> handlers::transfer_money,
        // [Method::GET, r"^/account/(?P<name>[\w]+)$", {"name"}] -> handlers::account,
        // _ -> handlers::not_found,
        // ```
        $([$match_method:expr, $match_regex:expr, {$($match_capture_name:expr),*}] -> $match_func:expr),*
        , _ -> $no_match_func:expr
        $(,),*
    ) => {
        $(
            {
                lazy_static::lazy_static! {
                    static ref REG: regex::Regex = regex::Regex::new($match_regex).unwrap();
                }
                if $method == $match_method {
                    if let Some(caps) = REG.captures($uri) {
                        slog::debug!(
                            LOG,
                            "router match";
                            "method" => $method.as_str(),
                            "match_uri" => $match_regex,
                            "uri" => $uri,
                        );
                        // `.captures` returns the `Some(caps)`, where
                        // caps[0] is the full match and all other
                        // captures start at index 1, or `None`
                        // if the regex wasn't a match
                        #[allow(unused_mut)]
                        let ctx = if caps.len() > 1 {
                            let mut map = std::collections::HashMap::new();
                            $(
                               let capture = caps
                                   .name($match_capture_name)
                                   .ok_or_else(|| {
                                       crate::error::ErrorKind::InvalidUriParam(
                                           format!(
                                               "uri parameter '{}' is not specified as a capture in '{}'",
                                               $match_capture_name, $match_regex,
                                           )
                                       )
                                   })?
                                   .as_str()
                                   .to_string();
                               map.insert($match_capture_name.to_string(), capture);
                            )*
                            crate::Context {
                                request: $request,
                                captures: crate::Caps::with(map),
                            }
                        } else {
                            crate::Context {
                                request: $request,
                                captures: crate::Caps::empty(),
                            }
                        };
                        return Ok($match_func(ctx).await?);
                    }
                }
            }
        )*
        let ctx = crate::Context {
            request: $request,
            captures: crate::Caps::empty(),
        };
        return Ok($no_match_func(ctx).await?);
    };
}
