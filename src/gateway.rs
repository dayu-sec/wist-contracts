//! Gateway envelope contract types.

use serde::{Deserialize, Serialize};

use crate::API_VERSION_V1;
use crate::action_plan::ActionPlan;
use crate::action_result::{ActionResult, FinalStatus};
use crate::discovery_policy::{DiscoveryAspectPolicy, DiscoveryAspectPolicySet};
use crate::enrollment::HostProfile;

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
#[serde(deny_unknown_fields)]
pub struct AgentWorkStateChange {
    pub input_id: String,
    pub state: AgentWorkState,
    pub reason: String,
    pub at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(
    kind = "message",
    role = "command",
    domain = "Reporting",
    module = "Reporting.Protocol"
)]
#[serde(deny_unknown_fields)]
pub struct AgentStatusReport {
    pub agent_id: String,
    pub instance_id: String,
    pub version: String,
    /// Own resident-set size in bytes reported by the agent.
    #[serde(default)]
    pub memory_bytes: Option<u64>,
    /// Agent 进程自身 CPU 占用，**单核口径**（100% = 占满一个核；多线程进程可 >100）。
    ///
    /// 它只统计 agent 进程**自己**的 CPU 时间（`getrusage(RUSAGE_SELF)`），不含它拉起的子进程。
    #[serde(default)]
    pub cpu_percent: Option<f64>,
    /// Agent 所在机器的**逻辑核数**（`available_parallelism`）。
    ///
    /// 为什么必须和 `cpu_percent` 同一份上报带上来：`cpu_percent` 是单核口径，
    /// 而运维看图时真正常问的是「这台机器被它占了百分之几」——那是 `cpu_percent / 核数`。
    /// 少了核数，右侧那个数既算不出来、也无法复核（4 核上的 13% 和 64 核上的 13% 完全不是一回事）。
    ///
    /// 为什么让 agent 报原始值、而不是它自己算好整机占比：沿用本仓已有的取舍
    /// （与 `discovery_policy_version`、事实摘要 digest 同理）——agent 只交**原始事实**
    /// （自己的 CPU 时间、自己的核数），换算只留一处，在网关。agent 自算的派生值
    /// 一旦算法退化，下游没有任何一层能发现。
    #[serde(default)]
    pub cpu_cores: Option<u32>,
    /// Measured round-trip latency to the admin control plane in milliseconds.
    #[serde(default)]
    pub admin_latency_ms: Option<u64>,
    /// 自上次上报以来的工作状态变化（paused/resumed），非告警、非失败。
    #[serde(default)]
    pub work_state_changes: Option<Vec<AgentWorkStateChange>>,
    /// 本机**实际生效**的发现方向策略版本；`None` = 还没拿到策略表（在用内建默认周期）。
    ///
    /// 为什么必须由 agent 上报、而不是网关自己记账：网关知道自己**发布**了哪一版，
    /// 但不知道某台机器**拉到并应用**了哪一版 —— 拉取可能失败、可能还没到轮询节拍、
    /// 也可能拿到后被夹取。而运维要回答的正是那句「我改了策略，哪些机器还没生效」。
    #[serde(default)]
    pub discovery_policy_version: Option<i64>,
    /// 本机**工作内容视图**（`state/work.json` 的子集）：我手里有哪些工作、各自在采哪些文件、
    /// 一次性工作做到哪一步。
    ///
    /// 为什么必须由 agent 上报：网关知道自己**授权**了什么，但「真的在采哪些文件」只有 agent
    /// 知道（本机手工加的输入、暂停、某条来源今天接不接得了）。网关侧只存最近一份。
    /// `None` = 这台 agent 还没报过（旧版本 agent 不发这个字段）。
    #[serde(default)]
    pub local_work: Option<crate::local_work::AgentLocalWork>,
    /// 本机**实际生效**的采集输出状态（见 [`crate::agent_uplink::AgentUplinkState`]）。
    ///
    /// 为什么必须由 agent 上报：网关知道自己**下发**了「启用 + 目标」，但不知道 agent
    /// **生效**成了什么 —— grant 可能还没拉到、可能被本机总闸拦住、可能目标连不上。
    /// 运维要回答的正是那句「这台为什么不上送」。
    ///
    /// `None` = 这台 agent 还没报过（旧版本 agent 不发这个字段）—— 落库后保持上一次的值
    /// （与 `local_work` 同口径）。
    #[serde(default)]
    pub uplink_state: Option<crate::agent_uplink::AgentUplinkState>,
    /// 本机**客户端证书**状态（mTLS）；还没有证书时 `None`。
    ///
    /// 为什么要 agent 上报：证书与到期时间只有本机知道（服务端在握手期就验完了，
    /// 而**过期证书根本进不来**）；而「哪些机器快到期 / 已过期需重装」正是运维要提前看到的
    /// （见 `docs/design/agent-identity-mtls.md` §5.5）。
    ///
    /// `None` = 这台 agent 还没报过 / 没证书 —— 落库后保持上一次的值（与其他可选字段同口径）。
    #[serde(default)]
    pub certificate_status: Option<AgentCertificateStatus>,
    /// 机器画像（机器名 / `node_id` / `machine_id` / 网卡地址）—— 注册表里「这是哪台机器」的展示来源。
    ///
    /// 为什么由状态上报带（而不是只靠注册）：**凭证书首触重建**登记时，机器画像全是空的
    /// （证书只承载稳定身份），原设计指望「后续状态上报补齐」，但那条通道以前没有这些字段 ——
    /// 于是经证书注册的机器在管理面永远只剩一个 ID。这里把注册时的 `HostProfile` 原样带上。
    ///
    /// `None` = 老版本 agentd 没带 —— 落库后保持上一次的值（与 `local_work` / `uplink_state` 同口径）。
    #[serde(default)]
    pub machine_profile: Option<HostProfile>,
}

/// agent 本地客户端证书状态（上报给网关，供页面/告警展示）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCertificateStatus {
    /// 证书到期时刻（RFC3339）。
    pub not_after: String,
    /// 距到期的剩余秒数（已过期为负）。
    pub remaining_seconds: i64,
    /// `valid` / `renew_due` / `expired`。
    pub state: String,
    /// agent 本机**最近一次续签判定**的结果（§5.5）。
    ///
    /// `None` = 老版本 agentd 不发（或本机台账一时读不到）—— 网关落库时**保留上一次的值**
    /// （与本报告里 `local_work` / `uplink_state` 同一口径）。
    #[serde(default)]
    pub last_renewal: Option<AgentCredentialRenewal>,
}

/// agent 本机**最近一次续签判定**的结果（§5.5）。
///
/// 续签是后台动作，**不记录就等于静默**：agentd 把本地台账（`identity/renewal.json`）原样带上来，
/// 网关只存 / 展示，不重算。与 [`AgentCertificateStatus`] 同口径：`None` = 还没报过。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentCredentialRenewal {
    /// `not_due` / `renewed` / `failed` / `needs_reinstall` / `revoked`（与 agentd 本地台账同口径）。
    pub outcome: String,
    /// 本次判定时刻（RFC3339）。
    pub checked_at: String,
    /// 人读细节（失败原因 / 续到了什么时候…）；无内容时为空串。
    pub detail: String,
    /// 续签后证书的到期时刻（RFC3339）；无证书时为空串。
    pub not_after: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(
    kind = "message",
    role = "command",
    domain = "Reporting",
    module = "Reporting.Protocol"
)]
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
#[jumo(
    kind = "message",
    role = "command",
    domain = "Reporting",
    module = "Reporting.Protocol"
)]
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
#[jumo(
    kind = "message",
    role = "command",
    domain = "Reporting",
    module = "Reporting.Protocol"
)]
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
    // ── 以下三个是**留痕/展示**字段：**不进内容摘要**，也不参与判重 ──
    //
    // 为什么不进摘要：摘要回答的是「内容变了没有」（幂等键与用途判据的输入）。
    // 机器名、IP 会因 DHCP/改名而变，但它们不影响「这台机器是干什么用的」——
    // 放进摘要会让每次换网就触发一次重报与重算。所以它们只用于展示与追溯。
    // 也正因如此，`fact-v1` 的字段集**没变**，不需要 bump 版本、不需要强制重报。
    /// 主机标识（发现里 `host` 方向的 `host.id`）。
    #[serde(default)]
    pub host_id: String,
    /// 主机名（`host.name`）。
    #[serde(default)]
    pub host_name: String,
    /// 网卡地址（每块网卡一条，形如 `en0 192.168.1.5/24`）。
    #[serde(default)]
    pub network_addresses: Vec<String>,
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
            host_id: String::new(),
            host_name: String::new(),
            network_addresses: Vec::new(),
            reported_at,
        }
    }

    /// 补上**留痕/展示**字段（不参与内容摘要与判重）。
    ///
    /// 为什么另开一个方法而不是给构造函数再加三个参数：那个函数已经有 13 个位置参数，
    /// 再加就是 16 个 —— 调用方只需错一次顺序，就会把主机名传成 os、把端口传成包名，
    /// 而这类错**不会报错**（都是 String/Vec<String>），只会静默写错数据。
    pub fn with_display(
        mut self,
        host_id: String,
        host_name: String,
        network_addresses: Vec<String>,
    ) -> Self {
        self.host_id = host_id;
        self.host_name = host_name;
        self.network_addresses = network_addresses;
        self
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

pub const POLL_DISCOVERY_POLICIES_KIND: &str = "poll_discovery_policies";

/// agentd → 网关：拉取**发现方向策略表**（控制面，复用 agent 凭据）。
///
/// 为什么是「拉」而不是网关推：agentd 没有入站监听（那要开端口、要证书、要处理公网可达），
/// 而策略是**幂等内容**（声明式、可重复拉取，与 PollWork 同类）—— 拉一次就够，不必重放。
///
/// 为什么不需要 `wait_ms`（PollControlCommands 有）：那是长轮询指令流；策略表按版本变化，
/// 轮询周期由 agentd 自己控（它知道自己能承受多密）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(
    kind = "message",
    role = "command",
    domain = "Reporting",
    module = "Reporting.Protocol"
)]
#[serde(deny_unknown_fields)]
pub struct PollDiscoveryPolicies {
    pub api_version: String,
    pub kind: String,
    pub agent_id: String,
    pub instance_id: String,
    pub requested_at: String,
}

/// 网关返回的策略表（对应模型 `Discovery.Probe.DiscoveryAspectPolicySet`）。
///
/// 带 `policy_version`：agentd 用它判断「这份与我手上的是不是同一版」，
/// 从而在版本未变时跳过重算与日志（而不是每次都重新应用一遍）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(
    kind = "message",
    role = "response",
    domain = "Reporting",
    module = "Reporting.Protocol"
)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryPoliciesReturned {
    pub policy_version: i64,
    pub published_at: String,
    pub policies: Vec<DiscoveryAspectPolicy>,
    pub returned_at: String,
}

impl DiscoveryPoliciesReturned {
    /// 从策略表组响应。
    ///
    /// 不回带 `agent_id`/`instance_id`（其它 ack 会带）：这不是「确认某次上报」，
    /// 而是「把当前版本的内容交给你」—— 身份由凭证本身表达，重复一份只会多一个会失配的字段。
    pub fn from_set(set: &DiscoveryAspectPolicySet, returned_at: String) -> Self {
        Self {
            policy_version: set.policy_version,
            published_at: set.published_at.clone(),
            policies: set.policies.clone(),
            returned_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> ActionPlan {
        serde_json::from_str(
            r#"{"api_version":"v1","kind":"action_plan",
                "meta":{"action_id":"act-1","request_id":"req-1","template_id":null,
                        "tenant_id":"t","environment_id":"e","plan_version":1,
                        "compiled_at":"2026-09-27T00:00:00Z","expires_at":"2026-09-28T00:00:00Z"},
                "target":{"agent_id":"agent-1","instance_id":null,"node_id":"n","host_name":null,
                          "platform":"macos","arch":"arm64","selectors":{}},
                "constraints":{"risk_level":"R1","approval_ref":null,"approval_mode":"not_required",
                               "requested_by":"admin","reason":null,"max_total_duration_ms":1000,
                               "step_timeout_default_ms":500,"execution_profile":"default",
                               "required_capabilities":[]},
                "program":{"entry":"s1","steps":[{"id":"s1","kind":"invoke","op":"shell"}]}}"#,
        )
        .expect("plan")
    }

    fn attestation() -> ResultAttestation {
        ResultAttestation {
            result_digest: "sha256:abc".to_string(),
            signature: "dev-placeholder:sig".to_string(),
            issued_by: "dev-placeholder:agent-1".to_string(),
            attested_at: "2026-09-27T00:00:02Z".to_string(),
        }
    }

    #[test]
    fn dispatch_action_plan_new_stamps_the_envelope() {
        let dispatch = DispatchActionPlan::new("disp-1".to_string(), plan());
        assert_eq!(dispatch.api_version, API_VERSION_V1);
        assert_eq!(dispatch.kind, DISPATCH_ACTION_PLAN_KIND);

        let json = serde_json::to_string(&dispatch).expect("encode");
        let back: DispatchActionPlan = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, dispatch);
    }

    #[test]
    fn action_plan_ack_builder_and_new_stamp_the_envelope() {
        let built =
            ActionPlanAck::builder("disp-1".to_string(), "act-1".to_string(), AckStatus::Queued)
                .plan_digest("sha256:plan".to_string())
                .agent_id("agent-1".to_string())
                .instance_id("inst-1".to_string())
                .queue_position(Some(3))
                .received_at("2026-09-27T00:00:00Z".to_string())
                .acknowledged_at("2026-09-27T00:00:01Z".to_string())
                .build();
        assert_eq!(built.api_version, API_VERSION_V1);
        assert_eq!(built.kind, ACTION_PLAN_ACK_KIND);
        assert_eq!(built.queue_position, Some(3));
        assert_eq!(built.reason_code, None);

        let constructed = ActionPlanAck::new(
            "disp-1".to_string(),
            "act-1".to_string(),
            "sha256:plan".to_string(),
            "agent-1".to_string(),
            "inst-1".to_string(),
            None,
            AckStatus::Accepted,
            "2026-09-27T00:00:00Z".to_string(),
            "2026-09-27T00:00:01Z".to_string(),
        );
        assert_eq!(constructed.kind, ACTION_PLAN_ACK_KIND);

        let json = serde_json::to_string(&constructed).expect("encode");
        let back: ActionPlanAck = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, constructed);
    }

    #[test]
    fn ack_status_uses_the_wire_names_and_rejects_unknown_variants() {
        for (status, name) in [
            (AckStatus::Accepted, "accepted"),
            (AckStatus::Rejected, "rejected"),
            (AckStatus::Queued, "queued"),
            (AckStatus::Duplicate, "duplicate"),
            (AckStatus::Stale, "stale"),
            (AckStatus::Busy, "busy"),
        ] {
            assert_eq!(
                serde_json::to_string(&status).unwrap(),
                format!("\"{name}\"")
            );
        }
        assert!(serde_json::from_str::<AckStatus>("\"unknown\"").is_err());
    }

    #[test]
    fn report_action_result_new_sets_kind_and_leaves_dispatch_absent() {
        let result = ActionResult::new(
            "act-1".to_string(),
            "exec-1".to_string(),
            FinalStatus::Succeeded,
        );
        let report = ReportActionResult::new(
            "rep-1".to_string(),
            "act-1".to_string(),
            1,
            FinalStatus::Succeeded,
            "exec-1".to_string(),
            "sha256:plan".to_string(),
            "agent-1".to_string(),
            "inst-1".to_string(),
            attestation(),
            "2026-09-27T00:00:02Z".to_string(),
            result,
        );
        assert_eq!(report.api_version, API_VERSION_V1);
        assert_eq!(report.kind, REPORT_ACTION_RESULT_KIND);
        assert_eq!(report.dispatch_id, None);

        let json = serde_json::to_string(&report).expect("encode");
        let back: ReportActionResult = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, report);
    }

    #[test]
    fn agent_work_state_uses_snake_case_on_the_wire() {
        assert_eq!(
            serde_json::to_string(&AgentWorkState::Paused).unwrap(),
            "\"paused\""
        );
        assert_eq!(
            serde_json::to_string(&AgentWorkState::Resumed).unwrap(),
            "\"resumed\""
        );
        let change = AgentWorkStateChange {
            input_id: "app".to_string(),
            state: AgentWorkState::Paused,
            reason: "spool_over_limit".to_string(),
            at: "2026-09-27T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&change).expect("encode");
        let back: AgentWorkStateChange = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, change);

        // 与同族的 AgentStatusReport 一致：拒绝未知字段，避免字段漂移静默通过。
        let mutated = json.replacen('{', "{\"extra\":1,", 1);
        assert!(serde_json::from_str::<AgentWorkStateChange>(&mutated).is_err());
    }

    #[test]
    fn a_minimal_agent_status_report_decodes_and_extra_keys_fail() {
        // 旧版 agent 只发三个必填字段；其余全是 #[serde(default)]。
        let json = r#"{"agent_id":"a","instance_id":"i","version":"0.1.5"}"#;
        let report: AgentStatusReport = serde_json::from_str(json).expect("decode");
        assert_eq!(report.memory_bytes, None);
        assert_eq!(report.cpu_percent, None);
        assert_eq!(report.work_state_changes, None);
        assert_eq!(report.discovery_policy_version, None);
        assert!(report.local_work.is_none());
        assert!(report.uplink_state.is_none());
        assert!(report.certificate_status.is_none());
        // 机器画像同样是可选的：旧版 agent 不带 -> None（网关落库时保持上一次的值）。
        assert!(report.machine_profile.is_none());

        assert!(
            serde_json::from_str::<AgentStatusReport>(
                r#"{"agent_id":"a","instance_id":"i","version":"v","nope":1}"#
            )
            .is_err()
        );
    }

    /// 机器画像随状态上报带上时能如实解码（机器名 / node_id / 网卡地址）。
    #[test]
    fn an_agent_status_report_carries_the_machine_profile() {
        let json = r#"{
            "agent_id":"a","instance_id":"i","version":"0.1.24",
            "machine_profile":{
                "node_id":"node-1","hostname":"host-1","os":"linux","arch":"x86_64",
                "machine_id":"mid-1","cloud_instance_id":null,"k8s_node_uid":null,
                "ip_addresses":["en0 192.168.1.5/24","10.8.0.2"]
            }
        }"#;
        let report: AgentStatusReport = serde_json::from_str(json).expect("decode");
        let profile = report.machine_profile.expect("machine_profile present");
        assert_eq!(profile.hostname, "host-1");
        assert_eq!(profile.node_id, "node-1");
        assert_eq!(profile.machine_id, "mid-1");
        assert_eq!(
            profile.ip_addresses,
            vec!["en0 192.168.1.5/24".to_string(), "10.8.0.2".to_string()]
        );
    }

    /// 证书状态里的「最近一次续签」是可选的，且拒绝未知字段：
    /// 旧 agentd 少发它必须仍能解码（上线顺序不一，网关不能因此 400）。
    #[test]
    fn certificate_status_last_renewal_is_optional_and_round_trips() {
        let legacy =
            r#"{"not_after":"2026-11-04T00:00:00Z","remaining_seconds":100,"state":"valid"}"#;
        let status: AgentCertificateStatus = serde_json::from_str(legacy).expect("decode");
        assert!(status.last_renewal.is_none());

        let json = r#"{"not_after":"2026-11-04T00:00:00Z","remaining_seconds":100,"state":"valid",
            "last_renewal":{"outcome":"renewed","checked_at":"2026-10-08T00:00:00Z",
            "detail":"credential renewed","not_after":"2026-11-04T00:00:00Z"}}"#;
        let status: AgentCertificateStatus = serde_json::from_str(json).expect("decode");
        assert_eq!(
            status.last_renewal.as_ref().expect("renewal").outcome,
            "renewed"
        );

        let encoded = serde_json::to_string(&status).expect("encode");
        let back: AgentCertificateStatus = serde_json::from_str(&encoded).expect("decode");
        assert_eq!(back, status);

        // 字段漂移（`last_renewal` 里多出未知键）必须显形。
        let drifted = r#"{"not_after":"x","remaining_seconds":0,"state":"valid",
            "last_renewal":{"outcome":"renewed","checked_at":"t","detail":"","not_after":"",
            "extra":1}}"#;
        assert!(serde_json::from_str::<AgentCertificateStatus>(drifted).is_err());
    }

    #[test]
    fn fact_summary_ack_status_uses_snake_case_and_rejects_unknown() {
        assert_eq!(
            serde_json::to_string(&FactSummaryAckStatus::Duplicate).unwrap(),
            "\"duplicate\""
        );
        assert!(serde_json::from_str::<FactSummaryAckStatus>("\"nope\"").is_err());
    }

    #[test]
    fn new_agent_facts_defaults_display_fields_then_with_display_fills_them() {
        let summary = ReportAgentFactSummary::new_agent_facts(
            "fact_1".to_string(),
            "agent-1".to_string(),
            "inst-1".to_string(),
            "fact-v1:sha256:abc".to_string(),
            7,
            "2026-09-27T00:00:00Z".to_string(),
            "macos".to_string(),
            "arm64".to_string(),
            3,
            vec!["/usr/bin/a".to_string()],
            Vec::new(),
            vec!["443".to_string()],
            "2026-09-27T00:00:01Z".to_string(),
        );
        assert_eq!(summary.kind, REPORT_AGENT_FACT_SUMMARY_KIND);
        assert!(summary.host_id.is_empty());
        assert!(summary.network_addresses.is_empty());

        let with_display = summary.with_display(
            "host-id".to_string(),
            "host-name".to_string(),
            vec!["en0 10.0.0.1/24".to_string()],
        );
        assert_eq!(with_display.host_id, "host-id");
        assert_eq!(with_display.host_name, "host-name");

        let json = serde_json::to_string(&with_display).expect("encode");
        let back: ReportAgentFactSummary = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, with_display);
    }

    #[test]
    fn discovery_policies_returned_from_set_copies_the_versioned_table() {
        let set = DiscoveryAspectPolicySet::new(
            4,
            "2026-09-27T00:00:00Z".to_string(),
            vec![DiscoveryAspectPolicy {
                aspect: "host".to_string(),
                default_interval_seconds: 900,
                min_interval_seconds: 60,
                max_interval_seconds: 3600,
                baseline: true,
                enabled_by_default: true,
                platforms: vec!["macos".to_string(), "linux".to_string()],
                yields: "os/arch".to_string(),
            }],
        );
        let returned =
            DiscoveryPoliciesReturned::from_set(&set, "2026-09-27T00:00:01Z".to_string());
        assert_eq!(returned.policy_version, 4);
        assert_eq!(returned.policies, set.policies);
        assert_eq!(returned.returned_at, "2026-09-27T00:00:01Z");

        let json = serde_json::to_string(&returned).expect("encode");
        let back: DiscoveryPoliciesReturned = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, returned);
    }

    #[test]
    fn poll_discovery_policies_round_trips_and_rejects_unknown_fields() {
        let poll = PollDiscoveryPolicies {
            api_version: API_VERSION_V1.to_string(),
            kind: POLL_DISCOVERY_POLICIES_KIND.to_string(),
            agent_id: "agent-1".to_string(),
            instance_id: "inst-1".to_string(),
            requested_at: "2026-09-27T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&poll).expect("encode");
        let back: PollDiscoveryPolicies = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, poll);

        let mutated = json.replacen('{', "{\"extra\":1,", 1);
        assert!(serde_json::from_str::<PollDiscoveryPolicies>(&mutated).is_err());
    }
}
