//! github_rest_pulls — GitHub REST v3 pull-request endpoints
//! (RFC-0004 Slice 3).
//!
//! Streaming direct-response WASM integration (same dispatch shape as
//! `git_refs_advertise`). One HttpEndpoint row covers the whole
//! `/api/v3/repos/{owner}/{repo}/pulls` subtree; this module routes on
//! the path tail:
//!   * `POST  .../pulls`               — `PullRequest.Create` + `Open`
//!   * `GET   .../pulls[?state=...]`   — projection (open|closed|all)
//!   * `GET   .../pulls/{n}`           — projection
//!   * `PATCH .../pulls/{n}`           — `Close` (title/body edits have
//!                                       no spec action — answered 422)
//!   * `POST  .../pulls/{n}/reviews`   — `Review.Create` +
//!                                       `Approve`/`RequestChanges`
//!   * `PUT   .../pulls/{n}/merge`     — `PullRequest.Merge`
//!                                       (`merge-conflict:` errors → 409)
//!
//! Mutations dispatch entity actions via outbound OData with the
//! resolved GitToken principal mirrored in the headers, so Cedar
//! evaluates the real caller (author self-approval stays denied, Merge
//! needs `pr:merge`). Two deliberate bridges run as the system
//! principal and are flagged in the module docs because the spec's
//! state machine has no GitHub equivalent: the `Open → UnderReview`
//! hop before a reviewer verdict (GitHub has no "start review"
//! transition; the spec's `RequestReview` is author-only by policy).

#![forbid(unsafe_code)]

extern crate alloc;

mod gh;
mod handlers;
mod http;
mod mutations;
mod odata;
mod reviews;
#[cfg(test)]
mod shape;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use genesis_git_auth::{AuthEnv, Principal};
use serde_json::Value;
use temper_wasm_sdk::http_stream::InboundHttp;
use temper_wasm_sdk::prelude::*;

pub(crate) const TEMPER_API: &str = "http://127.0.0.1:3000";
pub(crate) const SYSTEM_TENANT: &str = "default";
pub(crate) const SYSTEM_PRINCIPAL: &str = "github-rest-pulls";

temper_module! {
    fn run(ctx: Context) -> Result<Value> {
        let http_value = ctx
            .http_request
            .clone()
            .ok_or_else(|| "github_rest_pulls requires HttpEndpoint dispatch".to_string())?;
        let http: InboundHttp = serde_json::from_value(http_value)
            .map_err(|e| format!("http_request parse error: {e}"))?;

        let path = http::strip_query(&http.path).to_string();
        let Some(route) = Route::parse(&path, &http) else {
            return http::respond_error(&http, 404, "Not Found");
        };
        dispatch(&ctx, &http, &route)
    }
}

pub(crate) struct Route {
    pub owner: String,
    pub repo: String,
    pub kind: RouteKind,
}

pub(crate) enum RouteKind {
    Collection,
    Single(i64),
    Reviews(i64),
    Merge(i64),
}

impl Route {
    fn parse(path: &str, http: &InboundHttp) -> Option<Route> {
        let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
        if segments.len() < 6
            || segments[..3] != ["api", "v3", "repos"]
            || segments[5] != "pulls"
        {
            return None;
        }
        let owner = http
            .params
            .get("owner")
            .cloned()
            .unwrap_or_else(|| segments[3].to_string());
        let repo = http
            .params
            .get("repo")
            .cloned()
            .unwrap_or_else(|| segments[4].to_string());
        let kind = match segments.len() {
            6 => RouteKind::Collection,
            7 | 8 => {
                let number: i64 = segments[6].parse().ok().filter(|n| *n > 0)?;
                match (segments.len(), segments.get(7).copied()) {
                    (7, _) => RouteKind::Single(number),
                    (8, Some("reviews")) => RouteKind::Reviews(number),
                    (8, Some("merge")) => RouteKind::Merge(number),
                    _ => return None,
                }
            }
            _ => return None,
        };
        Some(Route { owner, repo, kind })
    }

    pub(crate) fn repository_id(&self) -> String {
        format!("rp-{}-{}", self.owner, self.repo)
    }
}

fn dispatch(ctx: &Context, http: &InboundHttp, route: &Route) -> Result<Value, String> {
    match (http.method.as_str(), &route.kind) {
        ("GET", RouteKind::Collection) => handlers::list_pulls(ctx, http, route),
        ("POST", RouteKind::Collection) => mutations::create_pull(ctx, http, route),
        ("GET", RouteKind::Single(n)) => handlers::get_pull(ctx, http, route, *n),
        ("PATCH", RouteKind::Single(n)) => mutations::patch_pull(ctx, http, route, *n),
        ("POST", RouteKind::Reviews(n)) => reviews::create_review(ctx, http, route, *n),
        ("PUT", RouteKind::Merge(n)) => mutations::merge_pull(ctx, http, route, *n),
        _ => http::respond_error(http, 404, "Not Found"),
    }
}

/// The token lookup must target the server actually handling this
/// request: the host-derived base, not a fixed port (a 3000-only
/// hardcode made token resolution silently degrade to anonymous on
/// any other port).
pub(crate) fn auth_env(api_base: &str) -> AuthEnv<'_> {
    AuthEnv {
        temper_api: api_base,
        tenant: SYSTEM_TENANT,
        system_principal: SYSTEM_PRINCIPAL,
    }
}

pub(crate) fn system_headers(api_base: &str) -> Vec<(String, String)> {
    let mut headers = Principal::system(&auth_env(api_base)).outbound_headers();
    headers.push(("X-Temper-Agent-Type".to_string(), "system".to_string()));
    headers
}

/// Fresh entity row id: content hash + wall clock, so repeated creates
/// of the same PR pair after a close get distinct rows. The clock is a
/// parameter so the function stays pure for host-side tests (the host
/// time FFI symbol only links on wasm32).
pub(crate) fn fresh_row_id(prefix: &str, seed: &str, millis: i64) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(format!("{seed}|{millis}").as_bytes());
    let mut hex = String::with_capacity(24);
    for byte in digest.iter().take(12) {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("{prefix}-{hex}")
}

pub(crate) fn query_param(http: &InboundHttp, key: &str) -> Option<String> {
    let qs = http.path.split_once('?')?.1;
    for pair in qs.split('&') {
        let mut it = pair.splitn(2, '=');
        let k = it.next()?;
        let v = it.next().unwrap_or("");
        if k == key {
            return Some(v.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeMap;

    fn parse(path: &str) -> Option<Route> {
        let http = InboundHttp {
            method: "GET".to_string(),
            path: path.to_string(),
            headers: Vec::new(),
            params: BTreeMap::new(),
            principal_id: None,
            request_body_handle: 0,
            response_body_handle: 0,
        };
        Route::parse(http::strip_query(&http.path), &http)
    }

    #[test]
    fn parses_collection_route() {
        let route = parse("/api/v3/repos/octo/hello/pulls").expect("route");
        assert!(matches!(route.kind, RouteKind::Collection));
        assert_eq!(route.repository_id(), "rp-octo-hello");
    }

    #[test]
    fn parses_single_and_subresources() {
        assert!(matches!(
            parse("/api/v3/repos/octo/hello/pulls/12").map(|r| r.kind),
            Some(RouteKind::Single(12))
        ));
        assert!(matches!(
            parse("/api/v3/repos/octo/hello/pulls/12/reviews").map(|r| r.kind),
            Some(RouteKind::Reviews(12))
        ));
        assert!(matches!(
            parse("/api/v3/repos/octo/hello/pulls/12/merge").map(|r| r.kind),
            Some(RouteKind::Merge(12))
        ));
    }

    #[test]
    fn rejects_non_numeric_and_unknown_subresources() {
        assert!(parse("/api/v3/repos/octo/hello/pulls/abc").is_none());
        assert!(parse("/api/v3/repos/octo/hello/pulls/0").is_none());
        assert!(parse("/api/v3/repos/octo/hello/pulls/12/commits").is_none());
    }

    #[test]
    fn query_param_reads_state_filter() {
        let http = InboundHttp {
            method: "GET".to_string(),
            path: "/api/v3/repos/o/r/pulls?state=closed&per_page=5".to_string(),
            headers: Vec::new(),
            params: BTreeMap::new(),
            principal_id: None,
            request_body_handle: 0,
            response_body_handle: 0,
        };
        assert_eq!(query_param(&http, "state").as_deref(), Some("closed"));
        assert!(query_param(&http, "missing").is_none());
    }

    #[test]
    fn fresh_row_ids_have_prefix_and_length() {
        let id = fresh_row_id("pr", "rp-octo-hello|refs/heads/a|refs/heads/b", 1_700_000_000);
        assert!(id.starts_with("pr-"));
        assert_eq!(id.len(), 3 + 24);
        let other = fresh_row_id("pr", "rp-octo-hello|refs/heads/a|refs/heads/b", 1_700_000_001);
        assert_ne!(id, other, "clock advance must change the row id");
    }
}
