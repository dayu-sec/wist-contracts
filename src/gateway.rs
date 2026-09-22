//! Gateway envelope contract types.

use serde::{Deserialize, Serialize};

use crate::API_VERSION_V1;
use crate::action_plan::ActionPlan;
use crate::action_result::{ActionResult, FinalStatus};

pub const DISPATCH_ACTION_PLAN_KIND: &str = "dispatch_action_plan";
pub const ACTION_PLAN_ACK_KIND: &str = "action_plan_ack";
pub const REPORT_ACTION_RESULT_KIND: &str = "report_action_result";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentWorkState {
    Paused,
    Resumed,
}

/// 工作状态变化（非告警、非失败）：暂停/恢复各上报一次。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentWorkStateChange {
    pub input_id: String,
    pub state: AgentWorkState,
    pub reason: String,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "message", role = "command", domain = "Reporting", module = "Reporting.Protocol")]
#[serde(deny_unknown_fields)]
pub struct AgentStatusReport {
    pub agent_id: String,
    pub instance_id: String,
    pub version: String,
    /// Own resident-set size in bytes reported by the agent.
    #[serde(default)]
    pub memory_bytes: Option<u64>,
    /// Agent process CPU usage as a percentage over the last report interval.
    #[serde(default)]
    pub cpu_percent: Option<f64>,
    /// Measured round-trip latency to the admin control plane in milliseconds.
    #[serde(default)]
    pub admin_latency_ms: Option<u64>,
    /// 自上次上报以来的工作状态变化（paused/resumed），非告警、非失败。
    #[serde(default)]
    pub work_state_changes: Option<Vec<AgentWorkStateChange>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "message", role = "command", domain = "Reporting", module = "Reporting.Protocol")]
#[serde(deny_unknown_fields)]
pub struct DispatchActionPlan {
    pub api_version: String,
    pub kind: String,
    pub dispatch_id: String,
    pub plan: ActionPlan,
}

impl DispatchActionPlan {
    pub fn new(dispatch_id: String, plan: ActionPlan) -> Self {
        Self {
            api_version: API_VERSION_V1.to_string(),
            kind: DISPATCH_ACTION_PLAN_KIND.to_string(),
            dispatch_id,
            plan,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "event", domain = "Reporting", module = "Reporting.Protocol")]
#[serde(deny_unknown_fields)]
pub struct ActionPlanAck {
    pub api_version: String,
    pub kind: String,
    pub dispatch_id: String,
    pub action_id: String,
    pub plan_digest: String,
    pub agent_id: String,
    pub instance_id: String,
    pub execution_id: Option<String>,
    pub ack_status: AckStatus,
    pub reason_code: Option<String>,
    pub reason_message: Option<String>,
    pub queue_position: Option<u64>,
    pub received_at: String,
    pub acknowledged_at: String,
}

impl ActionPlanAck {
    pub fn builder(
        dispatch_id: String,
        action_id: String,
        ack_status: AckStatus,
    ) -> ActionPlanAckBuilder {
        ActionPlanAckBuilder {
            dispatch_id,
            action_id,
            plan_digest: String::new(),
            agent_id: String::new(),
            instance_id: String::new(),
            execution_id: None,
            ack_status,
            reason_code: None,
            reason_message: None,
            queue_position: None,
            received_at: String::new(),
            acknowledged_at: String::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        dispatch_id: String,
        action_id: String,
        plan_digest: String,
        agent_id: String,
        instance_id: String,
        execution_id: Option<String>,
        ack_status: AckStatus,
        received_at: String,
        acknowledged_at: String,
    ) -> Self {
        Self::builder(dispatch_id, action_id, ack_status)
            .plan_digest(plan_digest)
            .agent_id(agent_id)
            .instance_id(instance_id)
            .execution_id(execution_id)
            .received_at(received_at)
            .acknowledged_at(acknowledged_at)
            .build()
    }
}

#[derive(Debug, Clone, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Protocol")]
pub struct ActionPlanAckBuilder {
    dispatch_id: String,
    action_id: String,
    plan_digest: String,
    agent_id: String,
    instance_id: String,
    execution_id: Option<String>,
    ack_status: AckStatus,
    reason_code: Option<String>,
    reason_message: Option<String>,
    queue_position: Option<u64>,
    received_at: String,
    acknowledged_at: String,
}

impl ActionPlanAckBuilder {
    pub fn plan_digest(mut self, plan_digest: String) -> Self {
        self.plan_digest = plan_digest;
        self
    }

    pub fn agent_id(mut self, agent_id: String) -> Self {
        self.agent_id = agent_id;
        self
    }

    pub fn instance_id(mut self, instance_id: String) -> Self {
        self.instance_id = instance_id;
        self
    }

    pub fn execution_id(mut self, execution_id: Option<String>) -> Self {
        self.execution_id = execution_id;
        self
    }

    pub fn reason_code(mut self, reason_code: Option<String>) -> Self {
        self.reason_code = reason_code;
        self
    }

    pub fn reason_message(mut self, reason_message: Option<String>) -> Self {
        self.reason_message = reason_message;
        self
    }

    pub fn queue_position(mut self, queue_position: Option<u64>) -> Self {
        self.queue_position = queue_position;
        self
    }

    pub fn received_at(mut self, received_at: String) -> Self {
        self.received_at = received_at;
        self
    }

    pub fn acknowledged_at(mut self, acknowledged_at: String) -> Self {
        self.acknowledged_at = acknowledged_at;
        self
    }

    pub fn build(self) -> ActionPlanAck {
        ActionPlanAck {
            api_version: API_VERSION_V1.to_string(),
            kind: ACTION_PLAN_ACK_KIND.to_string(),
            dispatch_id: self.dispatch_id,
            action_id: self.action_id,
            plan_digest: self.plan_digest,
            agent_id: self.agent_id,
            instance_id: self.instance_id,
            execution_id: self.execution_id,
            ack_status: self.ack_status,
            reason_code: self.reason_code,
            reason_message: self.reason_message,
            queue_position: self.queue_position,
            received_at: self.received_at,
            acknowledged_at: self.acknowledged_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "message", role = "command", domain = "Reporting", module = "Reporting.Protocol")]
#[serde(deny_unknown_fields)]
pub struct ReportActionResult {
    pub api_version: String,
    pub report_id: String,
    pub kind: String,
    pub dispatch_id: Option<String>,
    pub action_id: String,
    pub report_attempt: u32,
    pub final_status: FinalStatus,
    pub execution_id: String,
    pub plan_digest: String,
    pub agent_id: String,
    pub instance_id: String,
    pub result_attestation: ResultAttestation,
    pub reported_at: String,
    pub result: ActionResult,
}

impl ReportActionResult {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        report_id: String,
        action_id: String,
        report_attempt: u32,
        final_status: FinalStatus,
        execution_id: String,
        plan_digest: String,
        agent_id: String,
        instance_id: String,
        result_attestation: ResultAttestation,
        reported_at: String,
        result: ActionResult,
    ) -> Self {
        Self {
            api_version: API_VERSION_V1.to_string(),
            report_id,
            kind: REPORT_ACTION_RESULT_KIND.to_string(),
            dispatch_id: None,
            action_id,
            report_attempt,
            final_status,
            execution_id,
            plan_digest,
            agent_id,
            instance_id,
            result_attestation,
            reported_at,
            result,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Protocol")]
#[serde(deny_unknown_fields)]
pub struct ResultAttestation {
    /// Development placeholder until real signing and verifier plumbing is implemented.
    pub result_digest: String,
    /// Development placeholder signature. Consumers must not treat this as production attestation.
    pub signature: String,
    /// Development placeholder issuer identity, prefixed as `dev-placeholder:...`.
    pub issued_by: String,
    pub attested_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AckStatus {
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "rejected")]
    Rejected,
    #[serde(rename = "queued")]
    Queued,
    #[serde(rename = "duplicate")]
    Duplicate,
    #[serde(rename = "stale")]
    Stale,
    #[serde(rename = "busy")]
    Busy,
}

/// Gateway 对动作结果上报的确认响应。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionResultAck {
    pub report_id: String,
    pub agent_id: String,
    pub acknowledged_at: String,
}

/// Gateway 对 Agent 状态上报的确认响应。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentStatusAck {
    pub agent_id: String,
    pub instance_id: String,
    pub acknowledged_at: String,
}

pub const REPORT_AGENT_FACT_SUMMARY_KIND: &str = "report_agent_fact_summary";

/// 事实上报的确认状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactSummaryAckStatus {
    /// 已入库。
    Accepted,
    /// 内容未变（网关按**自算**摘要判定）：只刷留痕，未改内容、未重复计分。
    Duplicate,
    /// envelope 或身份非法。
    Rejected,
}

/// agentd → 网关的事实**摘要**上报（控制面）。
///
/// 与数据面上的原文快照（`ReportDiscoverySnapshot`）分工不同，**不是同一条路**：
/// 摘要只服务用途推断（网关侧按规则表算），去重后 10~30 KB，走已认证的控制面；
/// 原文快照一台几百 KB，走数据面给中心做资产整理。所以网关只接摘要。
///
/// 幂等键是内容摘要，不是 `revision`（后者每轮 refresh 无条件 +1）。
///
/// agentd **无条件周期全量**上报，判重归网关：网关用
/// `wist_contracts::fact_summary::FactContent::content_digest` 从收到的内容**自己算**摘要，
/// 以此判重。`content_digest` 字段因此只是 agent 的**声明**：
/// 与网关算出来的不一致时会记 `FactDigestMismatch` 告警（可能只是版本偏差，**不拒收**）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "message", role = "command", domain = "Reporting", module = "Reporting.Protocol")]
#[serde(deny_unknown_fields)]
pub struct ReportAgentFactSummary {
    pub api_version: String,
    pub kind: String,
    pub report_id: String,
    pub agent_id: String,
    pub instance_id: String,
    /// agent 侧声明的内容摘要。**不是**判重键：网关从下列内容字段自算，此值只作版本偏差的金丝雀。
    pub content_digest: String,
    /// 仅留痕：快照 revision 每轮 refresh 无条件 +1，网关不据它判重。
    pub revision: i64,
    /// 仅留痕：观察到的事实属于哪一刻（快照生成时间）。
    pub observed_at: String,
    pub os: String,
    pub arch: String,
    /// 仅留痕：去重前的进程条数（去重会毁掉基数，留一个原始计数备查），不进摘要。
    pub process_count: i64,
    /// 去重后的进程可执行标识。注意两边不同源：
    /// macOS 是 `ps -axo comm=` 给的完整路径，Linux 是 `/proc/{pid}/comm`（只有 basename）。
    pub process_executables: Vec<String>,
    /// 已装包名（仅 linux；macOS 侧待定）。
    pub packages: Vec<String>,
    pub listen_ports: Vec<String>,
    pub reported_at: String,
}

impl ReportAgentFactSummary {
    #[allow(clippy::too_many_arguments)]
    pub fn new_agent_facts(
        report_id: String,
        agent_id: String,
        instance_id: String,
        content_digest: String,
        revision: i64,
        observed_at: String,
        os: String,
        arch: String,
        process_count: i64,
        process_executables: Vec<String>,
        packages: Vec<String>,
        listen_ports: Vec<String>,
        reported_at: String,
    ) -> Self {
        Self {
            api_version: API_VERSION_V1.to_string(),
            kind: REPORT_AGENT_FACT_SUMMARY_KIND.to_string(),
            report_id,
            agent_id,
            instance_id,
            content_digest,
            revision,
            observed_at,
            os,
            arch,
            process_count,
            process_executables,
            packages,
            listen_ports,
            reported_at,
        }
    }
}

/// Gateway 对事实上报的确认响应（对应模型 `FactSummaryAccepted`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactSummaryAccepted {
    pub report_id: String,
    pub agent_id: String,
    pub content_digest: String,
    pub ack_status: FactSummaryAckStatus,
    /// 幂等命中（`duplicate`）时仍回带已存的建议，Agent 侧不必再问一次。
    pub suggestion_id: Option<String>,
    pub received_at: String,
}
