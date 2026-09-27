//! `ActionPlan` contract types.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::API_VERSION_V1;

pub const ACTION_PLAN_KIND: &str = "action_plan";
pub const STEP_KIND_INVOKE: &str = "invoke";
pub const STEP_KIND_BRANCH: &str = "branch";
pub const STEP_KIND_GUARD: &str = "guard";
pub const STEP_KIND_OUTPUT: &str = "output";
pub const STEP_KIND_ABORT: &str = "abort";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
#[serde(deny_unknown_fields)]
pub struct ActionPlan {
    pub api_version: String,
    pub kind: String,
    pub meta: ActionPlanMeta,
    pub target: ActionPlanTarget,
    pub constraints: ActionPlanConstraints,
    pub program: ActionPlanProgram,
}

impl ActionPlan {
    pub fn new(
        meta: ActionPlanMeta,
        target: ActionPlanTarget,
        constraints: ActionPlanConstraints,
        program: ActionPlanProgram,
    ) -> Self {
        Self {
            api_version: API_VERSION_V1.to_string(),
            kind: ACTION_PLAN_KIND.to_string(),
            meta,
            target,
            constraints,
            program,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
#[serde(deny_unknown_fields)]
pub struct ActionPlanMeta {
    pub action_id: String,
    pub request_id: String,
    pub template_id: Option<String>,
    pub tenant_id: String,
    pub environment_id: String,
    pub plan_version: i64,
    pub compiled_at: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
#[serde(deny_unknown_fields)]
pub struct ActionPlanTarget {
    pub agent_id: String,
    pub instance_id: Option<String>,
    pub node_id: String,
    pub host_name: Option<String>,
    pub platform: String,
    pub arch: String,
    #[serde(default)]
    pub selectors: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
#[serde(deny_unknown_fields)]
pub struct ActionPlanConstraints {
    pub risk_level: RiskLevel,
    pub approval_ref: Option<String>,
    pub approval_mode: ApprovalMode,
    pub requested_by: String,
    pub reason: Option<String>,
    pub max_total_duration_ms: u64,
    pub step_timeout_default_ms: u64,
    pub execution_profile: String,
    #[serde(default)]
    pub required_capabilities: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    #[serde(rename = "R0")]
    R0,
    #[serde(rename = "R1")]
    R1,
    #[serde(rename = "R2")]
    R2,
    #[serde(rename = "R3")]
    R3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalMode {
    #[serde(rename = "not_required")]
    NotRequired,
    #[serde(rename = "required")]
    Required,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
#[serde(deny_unknown_fields)]
pub struct ActionPlanProgram {
    pub entry: String,
    pub steps: Vec<ActionPlanStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
#[serde(deny_unknown_fields)]
pub struct ActionPlanStep {
    pub id: String,
    pub kind: String,
    pub op: Option<String>,
}

pub fn is_known_step_kind(kind: &str) -> bool {
    matches!(
        kind,
        STEP_KIND_INVOKE | STEP_KIND_BRANCH | STEP_KIND_GUARD | STEP_KIND_OUTPUT | STEP_KIND_ABORT
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::API_VERSION_V1;

    fn sample() -> ActionPlan {
        ActionPlan::new(
            ActionPlanMeta {
                action_id: "act-1".to_string(),
                request_id: "req-1".to_string(),
                template_id: None,
                tenant_id: "tenant-a".to_string(),
                environment_id: "env-a".to_string(),
                plan_version: 1,
                compiled_at: "2026-09-27T00:00:00Z".to_string(),
                expires_at: "2026-09-28T00:00:00Z".to_string(),
            },
            ActionPlanTarget {
                agent_id: "agent-1".to_string(),
                instance_id: None,
                node_id: "node-1".to_string(),
                host_name: None,
                platform: "macos".to_string(),
                arch: "arm64".to_string(),
                selectors: BTreeMap::new(),
            },
            ActionPlanConstraints {
                risk_level: RiskLevel::R1,
                approval_ref: None,
                approval_mode: ApprovalMode::NotRequired,
                requested_by: "admin".to_string(),
                reason: None,
                max_total_duration_ms: 60_000,
                step_timeout_default_ms: 5_000,
                execution_profile: "default".to_string(),
                required_capabilities: vec!["collect_logs".to_string()],
            },
            ActionPlanProgram {
                entry: "step-1".to_string(),
                steps: vec![ActionPlanStep {
                    id: "step-1".to_string(),
                    kind: STEP_KIND_INVOKE.to_string(),
                    op: Some("shell".to_string()),
                }],
            },
        )
    }

    #[test]
    fn new_stamps_the_contract_version_and_kind() {
        let plan = sample();
        assert_eq!(plan.api_version, API_VERSION_V1);
        assert_eq!(plan.kind, ACTION_PLAN_KIND);
    }

    #[test]
    fn action_plan_round_trips_and_rejects_unknown_fields() {
        let plan = sample();
        let json = serde_json::to_string(&plan).expect("encode");
        let back: ActionPlan = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, plan);

        let with_extra = json.replacen('{', "{\"extra\":1,", 1);
        assert!(serde_json::from_str::<ActionPlan>(&with_extra).is_err());
    }

    #[test]
    fn risk_and_approval_use_the_model_wire_names() {
        assert_eq!(serde_json::to_string(&RiskLevel::R0).unwrap(), "\"R0\"");
        assert_eq!(serde_json::to_string(&RiskLevel::R3).unwrap(), "\"R3\"");
        assert_eq!(
            serde_json::to_string(&ApprovalMode::NotRequired).unwrap(),
            "\"not_required\""
        );
        assert_eq!(
            serde_json::to_string(&ApprovalMode::Required).unwrap(),
            "\"required\""
        );
        // 封闭集：未知变体必须报错，不能静默接受。
        assert!(serde_json::from_str::<RiskLevel>("\"R9\"").is_err());
        assert!(serde_json::from_str::<ApprovalMode>("\"maybe\"").is_err());
    }

    #[test]
    fn known_step_kinds_are_exactly_the_closed_set() {
        for kind in [
            STEP_KIND_INVOKE,
            STEP_KIND_BRANCH,
            STEP_KIND_GUARD,
            STEP_KIND_OUTPUT,
            STEP_KIND_ABORT,
        ] {
            assert!(is_known_step_kind(kind), "{kind}");
        }
        assert!(!is_known_step_kind("nope"));
        assert!(!is_known_step_kind(""));
    }
}
