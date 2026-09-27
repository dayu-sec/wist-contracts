//! `ActionResult` contract types.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::API_VERSION_V1;

pub const ACTION_RESULT_KIND: &str = "action_result";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
#[serde(deny_unknown_fields)]
pub struct ActionResult {
    pub api_version: String,
    pub kind: String,
    pub action_id: String,
    pub execution_id: String,
    pub request_id: Option<String>,
    pub final_status: FinalStatus,
    pub exit_reason: Option<String>,
    pub step_records: Vec<StepRecord>,
    pub outputs: ActionOutputs,
    pub resource_usage: Option<ResourceUsage>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

impl ActionResult {
    pub fn new(action_id: String, execution_id: String, final_status: FinalStatus) -> Self {
        Self {
            api_version: API_VERSION_V1.to_string(),
            kind: ACTION_RESULT_KIND.to_string(),
            action_id,
            execution_id,
            request_id: None,
            final_status,
            exit_reason: None,
            step_records: Vec::new(),
            outputs: ActionOutputs::default(),
            resource_usage: None,
            started_at: None,
            finished_at: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinalStatus {
    #[serde(rename = "succeeded")]
    Succeeded,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "timed_out")]
    TimedOut,
    #[serde(rename = "rejected")]
    Rejected,
}

impl FinalStatus {
    pub fn as_state_name(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepRecord {
    pub step_id: String,
    pub attempt: u32,
    pub op: Option<String>,
    pub status: StepStatus,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub error_code: Option<String>,
    pub stdout_summary: Option<String>,
    pub stderr_summary: Option<String>,
    pub resource_usage: Option<ResourceUsage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    #[serde(rename = "started")]
    Started,
    #[serde(rename = "succeeded")]
    Succeeded,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "timed_out")]
    TimedOut,
    #[serde(rename = "skipped")]
    Skipped,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
#[serde(deny_unknown_fields)]
pub struct ActionOutputs {
    #[serde(default)]
    pub items: Vec<ActionOutputItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
#[serde(deny_unknown_fields)]
pub struct ActionOutputItem {
    pub name: String,
    pub value: Value,
    pub redacted: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceUsage {
    pub max_rss_bytes: Option<u64>,
    pub cpu_time_ms: Option<u64>,
    pub stdout_bytes: Option<u64>,
    pub stderr_bytes: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_stamps_version_kind_and_empty_defaults() {
        let result = ActionResult::new(
            "act-1".to_string(),
            "exec-1".to_string(),
            FinalStatus::Succeeded,
        );
        assert_eq!(result.api_version, API_VERSION_V1);
        assert_eq!(result.kind, ACTION_RESULT_KIND);
        assert!(result.step_records.is_empty());
        assert_eq!(result.outputs, ActionOutputs::default());
        assert_eq!(result.request_id, None);
        assert_eq!(result.resource_usage, None);
    }

    #[test]
    fn final_status_state_names_match_the_wire_names() {
        // `as_state_name` 与 serde 的 rename 必须一字不差：下游把前者写进状态文件，
        // 把后者写上线，两者分叉就只能靠人看出来。
        for (status, name) in [
            (FinalStatus::Succeeded, "succeeded"),
            (FinalStatus::Failed, "failed"),
            (FinalStatus::Cancelled, "cancelled"),
            (FinalStatus::TimedOut, "timed_out"),
            (FinalStatus::Rejected, "rejected"),
        ] {
            assert_eq!(status.as_state_name(), name);
            assert_eq!(
                serde_json::to_string(&status).unwrap(),
                format!("\"{name}\"")
            );
            assert_eq!(
                serde_json::from_str::<FinalStatus>(&format!("\"{name}\"")).unwrap(),
                status
            );
        }
        assert!(serde_json::from_str::<FinalStatus>("\"unknown\"").is_err());
        assert!(serde_json::from_str::<StepStatus>("\"unknown\"").is_err());
    }

    #[test]
    fn action_result_round_trips_with_nested_records_and_outputs() {
        let mut result = ActionResult::new(
            "act-1".to_string(),
            "exec-1".to_string(),
            FinalStatus::Failed,
        );
        result.step_records.push(StepRecord {
            step_id: "step-1".to_string(),
            attempt: 2,
            op: Some("shell".to_string()),
            status: StepStatus::Failed,
            started_at: "2026-09-27T00:00:00Z".to_string(),
            finished_at: Some("2026-09-27T00:00:01Z".to_string()),
            duration_ms: Some(1_000),
            error_code: Some("nonzero_exit".to_string()),
            stdout_summary: None,
            stderr_summary: Some("boom".to_string()),
            resource_usage: Some(ResourceUsage {
                max_rss_bytes: Some(1024),
                cpu_time_ms: None,
                stdout_bytes: Some(0),
                stderr_bytes: Some(4),
            }),
        });
        result.outputs.items.push(ActionOutputItem {
            name: "file".to_string(),
            value: serde_json::json!({"path": "/tmp/out"}),
            redacted: Some(true),
        });

        let json = serde_json::to_string(&result).expect("encode");
        let back: ActionResult = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, result);
    }

    #[test]
    fn action_result_rejects_unknown_fields() {
        let result = ActionResult::new(
            "act-1".to_string(),
            "exec-1".to_string(),
            FinalStatus::Succeeded,
        );
        let json = serde_json::to_string(&result).expect("encode");
        let mutated = json.replacen('{', "{\"extra\":1,", 1);
        assert!(serde_json::from_str::<ActionResult>(&mutated).is_err());
    }
}
