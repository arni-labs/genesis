//! github_rest_refs — GitHub REST v3 branch and git-reference
//! endpoints (RFC-0004 Slice 2).
//!
//! Streaming direct-response WASM integration (same dispatch shape as
//! `git_refs_advertise`):
//!   * `GET    /api/v3/repos/{o}/{r}/branches`                 — Ref projection (refs/heads/*)
//!   * `GET    /api/v3/repos/{o}/{r}/git/refs[/{ref...}]`      — list / prefix-matched refs
//!   * `GET    /api/v3/repos/{o}/{r}/git/ref/{ref...}`         — single exact ref
//!   * `GET    /api/v3/repos/{o}/{r}/git/matching-refs/{ref...}` — prefix-matched refs
//!   * `POST   /api/v3/repos/{o}/{r}/git/refs`                 — `Ref.Create`
//!   * `PATCH  /api/v3/repos/{o}/{r}/git/refs/{ref...}`        — `Ref.Update` / `Ref.ForceUpdate`
//!   * `DELETE /api/v3/repos/{o}/{r}/git/refs/{ref...}`        — `Ref.Delete`
//!
//! PATCH semantics (documented divergence): GitHub requires
//! fast-forward when `force:false`. The commit-DAG ancestry walk lives
//! in the push path (`scm_ingest_pack`, ADR-0025); this REST update
//! dispatches `Ref.Update` with the freshly-read tip as
//! `PreviousCommitSha`, so the CAS precondition rejects concurrent
//! racers but does not prove ancestry. `force:true` maps to
//! `Ref.ForceUpdate`, which Cedar additionally gates on the `force`
//! token scope.

#![forbid(unsafe_code)]

extern crate alloc;

mod gh;
mod handlers;
mod http;
mod odata;
#[cfg(test)]
mod shape;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use genesis_git_auth::AuthEnv;
use serde_json::Value;
use temper_wasm_sdk::http_stream::InboundHttp;
use temper_wasm_sdk::prelude::*;

pub(crate) const TEMPER_API: &str = "http://127.0.0.1:3000";
pub(crate) const SYSTEM_TENANT: &str = "default";
pub(crate) const SYSTEM_PRINCIPAL: &str = "github-rest-refs";

temper_module! {
    fn run(ctx: Context) -> Result<Value> {
        let http_value = ctx
            .http_request
            .clone()
            .ok_or_else(|| "github_rest_refs requires HttpEndpoint dispatch".to_string())?;
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
    Branches,
    /// `git/refs` with an optional short-ref tail (e.g. `heads/main`).
    GitRefs(Option<String>),
    /// `git/ref/{tail}` — exact lookup.
    GitRef(String),
    /// `git/matching-refs/{tail}` — prefix match.
    MatchingRefs(String),
}

impl Route {
    fn parse(path: &str, http: &InboundHttp) -> Option<Route> {
        let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
        if segments.len() < 6 || segments[..3] != ["api", "v3", "repos"] {
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
        let kind = match segments[5] {
            "branches" if segments.len() == 6 => RouteKind::Branches,
            "git" if segments.len() >= 7 => {
                let tail = if segments.len() > 7 {
                    Some(segments[7..].join("/"))
                } else {
                    None
                };
                match (segments[6], tail) {
                    ("refs", tail) => RouteKind::GitRefs(tail),
                    ("ref", Some(tail)) => RouteKind::GitRef(tail),
                    ("matching-refs", Some(tail)) => RouteKind::MatchingRefs(tail),
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
        ("GET", RouteKind::Branches) => handlers::list_branches(ctx, http, route),
        ("GET", RouteKind::GitRefs(tail)) => {
            handlers::list_refs(ctx, http, route, tail.as_deref())
        }
        ("GET", RouteKind::MatchingRefs(tail)) => {
            handlers::list_refs(ctx, http, route, Some(tail))
        }
        ("GET", RouteKind::GitRef(tail)) => handlers::get_single_ref(ctx, http, route, tail),
        ("POST", RouteKind::GitRefs(None)) => handlers::create_ref(ctx, http, route),
        ("PATCH", RouteKind::GitRefs(Some(tail))) => {
            handlers::update_ref(ctx, http, route, tail)
        }
        ("DELETE", RouteKind::GitRefs(Some(tail))) => {
            handlers::delete_ref(ctx, http, route, tail)
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeMap;

    fn inbound(method: &str, path: &str) -> InboundHttp {
        InboundHttp {
            method: method.to_string(),
            path: path.to_string(),
            headers: Vec::new(),
            params: BTreeMap::new(),
            principal_id: None,
            request_body_handle: 0,
            response_body_handle: 0,
        }
    }

    fn parse(method: &str, path: &str) -> Option<Route> {
        let http = inbound(method, path);
        Route::parse(http::strip_query(&http.path), &http)
    }

    #[test]
    fn parses_branches_route() {
        let route = parse("GET", "/api/v3/repos/octo/hello/branches").expect("route");
        assert!(matches!(route.kind, RouteKind::Branches));
        assert_eq!(route.repository_id(), "rp-octo-hello");
    }

    #[test]
    fn parses_git_refs_with_multisegment_tail() {
        let route =
            parse("PATCH", "/api/v3/repos/octo/hello/git/refs/heads/feature/x").expect("route");
        match route.kind {
            RouteKind::GitRefs(Some(tail)) => assert_eq!(tail, "heads/feature/x"),
            _ => panic!("expected git/refs tail"),
        }
    }

    #[test]
    fn parses_bare_git_refs_collection() {
        let route = parse("POST", "/api/v3/repos/octo/hello/git/refs").expect("route");
        assert!(matches!(route.kind, RouteKind::GitRefs(None)));
    }

    #[test]
    fn parses_single_ref_lookup() {
        let route = parse("GET", "/api/v3/repos/octo/hello/git/ref/heads/main").expect("route");
        match route.kind {
            RouteKind::GitRef(tail) => assert_eq!(tail, "heads/main"),
            _ => panic!("expected git/ref tail"),
        }
    }

    #[test]
    fn rejects_bare_single_ref_lookup() {
        assert!(parse("GET", "/api/v3/repos/octo/hello/git/ref").is_none());
    }

    #[test]
    fn parses_matching_refs() {
        let route =
            parse("GET", "/api/v3/repos/octo/hello/git/matching-refs/heads/feat").expect("route");
        assert!(matches!(route.kind, RouteKind::MatchingRefs(t) if t == "heads/feat"));
    }
}
