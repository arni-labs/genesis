//! Trigger-context parsing: which spec action invoked the module and
//! what it asked for. Handles both `PullRequest.Merge` (entity = the
//! PR) and `Repository.MergePullRequest` (entity = the repository),
//! normalizing strategy names from ADR-0024 and GitHub's
//! `merge_method` vocabulary.

use alloc::format;
use alloc::string::{String, ToString};

use serde_json::Value;
use temper_wasm_sdk::prelude::*;

/// Merge strategy, normalized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Strategy {
    Merge,
    Squash,
    FastForward,
}

impl Strategy {
    pub(crate) fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            // GitHub's default merge_method when omitted is "merge".
            "" | "merge" => Ok(Strategy::Merge),
            "squash" => Ok(Strategy::Squash),
            "ff" | "fast-forward" | "fast_forward" | "fastforward" => Ok(Strategy::FastForward),
            "rebase" => Err(
                "merge strategy 'rebase' is not supported by Genesis v1 (ADR-0024); \
                 use 'merge', 'squash', or 'ff'"
                    .to_string(),
            ),
            other => Err(format!(
                "unknown merge strategy '{other}'; use 'merge', 'squash', or 'ff'"
            )),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Strategy::Merge => "merge",
            Strategy::Squash => "squash",
            Strategy::FastForward => "ff",
        }
    }
}

/// Which spec trigger invoked the module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Trigger {
    /// `PullRequest.Merge` — the entity is the PR; the kernel already
    /// gated the Approved → Merged transition.
    PullRequest { pr_id: String },
    /// `Repository.MergePullRequest` — the entity is the repository;
    /// the PR transition is emitted as a sub-write.
    Repository {
        repository_id: String,
        pr_id: Option<String>,
        pr_number: Option<u64>,
    },
}

/// Everything the trigger context determines about this merge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MergeRequest {
    pub(crate) trigger: Trigger,
    pub(crate) strategy: Strategy,
    pub(crate) message: Option<String>,
    pub(crate) client_request_id: String,
    pub(crate) committer_identity: Option<String>,
    pub(crate) timestamp_override: Option<(String, String)>,
}

impl MergeRequest {
    pub(crate) fn from_context(ctx: &Context) -> Result<Self, String> {
        let params = &ctx.trigger_params;
        let trigger = match (ctx.entity_type.as_str(), ctx.trigger_action.as_str()) {
            ("PullRequest", _) | (_, "Merge") => Trigger::PullRequest {
                pr_id: require_entity_id(ctx)?,
            },
            ("Repository", _) | (_, "MergePullRequest") => Trigger::Repository {
                repository_id: require_entity_id(ctx)?,
                pr_id: param_str(params, &["PullRequestId", "pull_request_id"]),
                pr_number: param_u64(params, &["PullRequestNumber", "Number", "number"]),
            },
            (entity, action) => {
                return Err(format!(
                    "scm_merge_pr: unsupported trigger {entity}.{action}"
                ));
            }
        };
        let strategy = Strategy::parse(
            &param_str(params, &["Strategy", "strategy", "MergeMethod", "merge_method"])
                .unwrap_or_default(),
        )?;
        let timestamp_override = parse_timestamp_override(params)?;
        Ok(MergeRequest {
            trigger,
            strategy,
            message: param_str(params, &["Message", "message", "CommitMessage", "commit_message"])
                .filter(|m| !m.is_empty()),
            client_request_id: param_str(params, &["ClientRequestId", "client_request_id"])
                .unwrap_or_default(),
            committer_identity: param_str(params, &["Committer", "CommitterIdentity"])
                .filter(|c| !c.is_empty()),
            timestamp_override,
        })
    }
}

fn require_entity_id(ctx: &Context) -> Result<String, String> {
    if ctx.entity_id.is_empty() {
        return Err("scm_merge_pr: trigger context has no entity_id".to_string());
    }
    Ok(ctx.entity_id.clone())
}

fn param_str(params: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| params.get(*key).and_then(Value::as_str))
        .map(ToString::to_string)
}

fn param_u64(params: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        let value = params.get(*key)?;
        value
            .as_u64()
            .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
    })
}

/// Optional deterministic timestamp from trigger params:
/// `CommitTimestamp` (unix seconds) + `CommitTimezone` (±HHMM,
/// default +0000). When absent the head commit's committer timestamp
/// is used instead — never a wall clock (see `commits` module docs).
fn parse_timestamp_override(params: &Value) -> Result<Option<(String, String)>, String> {
    let Some(raw) = params.get("CommitTimestamp").or_else(|| params.get("commit_timestamp"))
    else {
        return Ok(None);
    };
    let seconds = match raw {
        Value::Number(n) => n
            .as_u64()
            .map(|n| n.to_string())
            .ok_or_else(|| "CommitTimestamp must be a non-negative integer".to_string())?,
        Value::String(s) if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) => s.clone(),
        _ => return Err("CommitTimestamp must be unix seconds (number or digits)".to_string()),
    };
    let timezone = param_str(params, &["CommitTimezone", "commit_timezone"])
        .unwrap_or_else(|| "+0000".to_string());
    let tz_ok = timezone.len() == 5
        && (timezone.starts_with('+') || timezone.starts_with('-'))
        && timezone[1..].chars().all(|c| c.is_ascii_digit());
    if !tz_ok {
        return Err(format!("CommitTimezone '{timezone}' is not ±HHMM"));
    }
    Ok(Some((seconds, timezone)))
}
