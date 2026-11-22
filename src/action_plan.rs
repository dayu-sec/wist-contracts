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

/// 计划内容摘要的版本前缀（形状变了就升版本，不要悄悄改）。
pub const PLAN_DIGEST_VERSION: &str = "plan-v1";
/// 第一个（也是目前唯一）的签名算法。
pub const ATTESTATION_ALGORITHM_ED25519: &str = "ed25519";

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
    /// 签发证明：**不在摘要/签名范围内**（否则自我引用）。
    ///
    /// `None` = 未签发（E2 的直接下发路径）；执行侧是否接受由本地策略决定
    /// （`attestation_enforced`），而不是由「有没有这个字段」决定。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation: Option<ActionPlanAttestation>,
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
            attestation: None,
        }
    }
}

/// 计划内容的**规范化摘要**：`plan-v1:sha256:<hex>`。
///
/// 规范化靠「序列化成 Value → 递归排序对象键 → 紧凑输出」，**不另写一份镜像结构体**：
/// 后者一旦忘了同步新字段，新字段就不在摘要范围内 —— 那是静默的安全洞。
/// `attestation` 必须排除：它是签名的**载体**，不是被签的内容。
pub fn plan_digest(plan: &ActionPlan) -> String {
    let mut value = serde_json::to_value(plan).expect("action plan serialization is infallible");
    if let serde_json::Value::Object(map) = &mut value {
        map.remove("attestation");
    }
    // `args: null` 与「没有 args」在执行侧**完全等价**（`args.unwrap_or(&null)`），所以摘要
    // 也必须等价。否则同一份计划经过一次 JSON 往返（`Some(Null)` → `None`）就会判成
    // “内容被改过”—— 而实际上一个字节都没变。
    if let Some(steps) = value
        .get_mut("program")
        .and_then(|program| program.get_mut("steps"))
        .and_then(serde_json::Value::as_array_mut)
    {
        for step in steps {
            if let serde_json::Value::Object(step) = step
                && step.get("args").is_some_and(serde_json::Value::is_null)
            {
                step.remove("args");
            }
        }
    }
    let canonical = canonical_json(&value);
    format!(
        "{PLAN_DIGEST_VERSION}:sha256:{}",
        sha256_hex(canonical.as_bytes())
    )
}

/// 规范 JSON：对象键按字典序、无空白、数组保持顺序。
///
/// 三端（签、转发、验）必须算出同一串字节，否则现场只会看到「每天验签失败」。
pub fn canonical_json(value: &serde_json::Value) -> String {
    use serde_json::Value;
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        // `Number` 的 Display 对整数/浮点都是最短确定表示。
        Value::Number(value) => value.to_string(),
        Value::String(value) => {
            serde_json::to_string(value).expect("string serialization is infallible")
        }
        Value::Array(items) => {
            let mut out = String::from("[");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&canonical_json(item));
            }
            out.push(']');
            out
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_unstable();
            let mut out = String::from("{");
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).expect("key serialization is infallible"));
                out.push(':');
                out.push_str(&canonical_json(&map[key]));
            }
            out.push('}');
            out
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// 签发证明：回答「谁批的、批的是不是这一份」。
///
/// 被签的字节 = 本结构的规范化 JSON **去掉 `signature`**（见 [`Self::signing_payload`]）。
/// 这样新加字段自动进签名范围，不存在「加了字段但忘了签」的洞。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
#[serde(deny_unknown_fields)]
pub struct ActionPlanAttestation {
    pub api_version: String,
    /// 计划内容的摘要（与 [`plan_digest`] 同口径）。
    pub plan_digest: String,
    pub approval: ActionPlanApproval,
    /// 审批时计划声明的风险等级（执行侧必须比对，防止“批了 R1、跑 R2”）。
    pub risk_level: RiskLevel,
    pub issued_by: String,
    pub key_id: String,
    pub algorithm: String,
    pub signed_at: String,
    pub expires_at: String,
    /// 小写 hex（Ed25519 = 128 个字符）。
    pub signature: String,
}

impl ActionPlanAttestation {
    /// 被签的字节：本结构去掉 `signature` 后的规范 JSON。
    pub fn signing_payload(&self) -> Vec<u8> {
        let mut value =
            serde_json::to_value(self).expect("attestation serialization is infallible");
        if let serde_json::Value::Object(map) = &mut value {
            map.remove("signature");
        }
        canonical_json(&value).into_bytes()
    }
}

/// 审批事实（**签名覆盖**，所以执行侧不用问中心也能判断“够不够人”）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
#[serde(deny_unknown_fields)]
pub struct ActionPlanApproval {
    pub approval_id: String,
    /// 审批人的标识（人与机器的命名空间分开，例如 `user:alice`）。
    pub approvers: Vec<String>,
    /// 策略要求的**最少**审批人数（R2 = 1，R3 = 2）。
    pub required_approvers: u8,
    pub approved_at: String,
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
    /// 显式允许的路径前缀（需要路径参数的 op 必须先用它过一层）。
    ///
    /// `None` / 空列表 = **什么路径都不许**——不是“什么都可以”。默认拒绝是刻意的：
    /// 漏了它只会在执行时报 `path_not_allowed`，而不会是“默默地读了整台机器”。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow: Option<ActionPlanAllow>,
}

/// 路径白名单。目前只有一个维度；将来如果出现“只读某类文件”，在这里加闭集，
/// 而不是给 opcode 加自由参数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
#[serde(deny_unknown_fields)]
pub struct ActionPlanAllow {
    #[serde(default)]
    pub paths: Vec<String>,
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
    /// `kind = invoke` 的参数；缺省 = 无参数。
    ///
    /// 用 `Value` 而不是强类型：每个 opcode 的参数形状不同（`process.list` 与
    /// `disk.cleanup.apply` 没有共同字段），由 opcode 注册表在**执行侧**校验。
    /// 契约层只保证它是 JSON 对象或 `null`（见 `wist-validate` 的 `invalid_step_args`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
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
                allow: None,
            },
            ActionPlanProgram {
                entry: "step-1".to_string(),
                steps: vec![ActionPlanStep {
                    id: "step-1".to_string(),
                    kind: STEP_KIND_INVOKE.to_string(),
                    op: Some("shell".to_string()),
                    args: None,
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
    fn step_args_are_optional_and_round_trip() {
        // 缺省 `args` 的旧计划：解成 `None`，重新编码时**不出现**这个键。
        let mut plan = sample();
        let json = serde_json::to_string(&plan).expect("encode");
        assert!(
            !json.contains("\"args\""),
            "absent args must not be serialized: {json}"
        );

        // 带 `args` 的计划可 round-trip，嵌套值原样保留。
        plan.program.steps[0].args = Some(serde_json::json!({
            "target": "tmp",
            "max_files": 100,
            "nested": { "a": [1, 2] }
        }));
        let json = serde_json::to_string(&plan).expect("encode");
        let back: ActionPlan = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, plan);
        assert_eq!(
            back.program.steps[0].args.as_ref().unwrap()["target"],
            "tmp"
        );
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

    fn attestation(signature: &str) -> ActionPlanAttestation {
        ActionPlanAttestation {
            api_version: API_VERSION_V1.to_string(),
            plan_digest: "plan-v1:sha256:0000".to_string(),
            approval: ActionPlanApproval {
                approval_id: "apv-1".to_string(),
                approvers: vec!["user:alice".to_string()],
                required_approvers: 1,
                approved_at: "2026-11-22T00:00:00Z".to_string(),
            },
            risk_level: RiskLevel::R2,
            issued_by: "gateway".to_string(),
            key_id: "k1".to_string(),
            algorithm: ATTESTATION_ALGORITHM_ED25519.to_string(),
            signed_at: "2026-11-22T00:00:00Z".to_string(),
            expires_at: "2026-11-22T00:15:00Z".to_string(),
            signature: signature.to_string(),
        }
    }

    #[test]
    fn canonical_json_sorts_keys_and_keeps_array_order() {
        let a: serde_json::Value = serde_json::from_str(r#"{"b":1,"a":[3,2]}"#).unwrap();
        let b: serde_json::Value = serde_json::from_str(r#"{"a":[3,2],"b":1}"#).unwrap();
        assert_eq!(canonical_json(&a), canonical_json(&b));
        assert_eq!(canonical_json(&a), r#"{"a":[3,2],"b":1}"#);
        // 数组顺序是语义，不许排。
        let c: serde_json::Value = serde_json::from_str(r#"{"a":[2,3],"b":1}"#).unwrap();
        assert_ne!(canonical_json(&a), canonical_json(&c));
    }

    #[test]
    fn the_plan_digest_ignores_the_attestation_but_covers_the_content() {
        let base = sample();
        let digest = plan_digest(&base);
        assert!(digest.starts_with("plan-v1:sha256:"), "{digest}");

        // 同一份计划 → 同一个摘要；带不带 attestation 都一样（否则签名自我引用）。
        let mut with_attestation = base.clone();
        with_attestation.attestation = Some(attestation("aa"));
        assert_eq!(plan_digest(&with_attestation), digest);
        let mut other_signature = base.clone();
        other_signature.attestation = Some(attestation("bb"));
        assert_eq!(plan_digest(&other_signature), digest);

        // 内容变了就必须变 —— 覆盖 steps 的 args、constraints 的 allow、meta 的过期时间。
        let mut changed_args = base.clone();
        changed_args.program.steps[0].args = Some(serde_json::json!({ "target": "tmp" }));
        assert_ne!(plan_digest(&changed_args), digest);

        let mut changed_allow = base.clone();
        changed_allow.constraints.allow = Some(ActionPlanAllow {
            paths: vec!["/tmp".to_string()],
        });
        assert_ne!(plan_digest(&changed_allow), digest);

        let mut changed_risk = base.clone();
        changed_risk.constraints.risk_level = RiskLevel::R3;
        assert_ne!(plan_digest(&changed_risk), digest);
    }

    #[test]
    fn args_key_order_does_not_change_the_digest() {
        let mut first = sample();
        first.program.steps[0].args = Some(serde_json::from_str(r#"{"a":1,"b":2}"#).unwrap());
        let mut second = sample();
        second.program.steps[0].args = Some(serde_json::from_str(r#"{"b":2,"a":1}"#).unwrap());
        assert_eq!(plan_digest(&first), plan_digest(&second));
    }

    #[test]
    fn canonical_json_is_deterministic_for_numbers_and_escapes() {
        // 数字按 serde_json 的最短确定表示；签 / 转发 / 验三端必须算出同一串字节。
        assert_eq!(canonical_json(&serde_json::json!(1)), "1");
        assert_eq!(canonical_json(&serde_json::json!(-42)), "-42");
        assert_eq!(canonical_json(&serde_json::json!(1.5)), "1.5");
        assert_eq!(canonical_json(&serde_json::json!(1.0)), "1.0");
        // 字符串按 JSON 规则转义；非 ASCII **不**转成 \u（否则两端转义策略不同就会漂）。
        assert_eq!(
            canonical_json(&serde_json::json!("a\"b\n")),
            "\"a\\\"b\\n\""
        );
        assert_eq!(canonical_json(&serde_json::json!("中文")), "\"中文\"");
    }

    /// `args: null` 与「没有 args」在执行侧等价（`unwrap_or(&null)`），摘要必须也等价。
    /// 否则一次 JSON 往返就能让同一份计划在验签时被判成“内容被改过”。
    #[test]
    fn an_explicit_null_args_is_the_same_plan_as_no_args() {
        let mut absent = sample();
        absent.program.steps[0].args = None;
        let mut explicit_null = sample();
        explicit_null.program.steps[0].args = Some(serde_json::Value::Null);
        assert_eq!(plan_digest(&absent), plan_digest(&explicit_null));
    }

    #[test]
    fn the_signing_payload_excludes_only_the_signature() {
        let first = attestation("aa");
        let second = attestation("bb");
        assert_eq!(first.signing_payload(), second.signing_payload());

        let mut changed = first.clone();
        changed.risk_level = RiskLevel::R3;
        assert_ne!(first.signing_payload(), changed.signing_payload());

        let mut changed_key = first.clone();
        changed_key.key_id = "k2".to_string();
        assert_ne!(first.signing_payload(), changed_key.signing_payload());

        // payload 里不得出现 signature 字段本身。
        let payload = String::from_utf8(first.signing_payload()).unwrap();
        assert!(!payload.contains("signature"), "{payload}");
        assert!(!payload.contains("\"aa\""), "{payload}");
    }

    #[test]
    fn a_plan_without_attestation_serializes_without_the_key() {
        let json = serde_json::to_value(sample()).unwrap();
        assert!(json.get("attestation").is_none());
        // 带 attestation 时也得能解回来（deny_unknown_fields 下这是加性字段）。
        let mut plan = sample();
        plan.attestation = Some(attestation("aa"));
        let text = serde_json::to_string(&plan).unwrap();
        let decoded: ActionPlan = serde_json::from_str(&text).expect("round trip");
        assert_eq!(decoded.attestation, plan.attestation);
    }

    /// 黄金向量：三端（center / gateway / agentd）必须算出同一个摘要。
    /// 这个值一旦因“重构”而变，就说明规范化规则被动过 —— 那是破坏性变更。
    #[test]
    fn the_plan_digest_is_a_golden_vector() {
        assert_eq!(
            plan_digest(&sample()),
            "plan-v1:sha256:a57a1ac0ef79ddb72b41bf32d5dd57884865d9f5dd97719c3ed5742397242108"
        );
    }
}
