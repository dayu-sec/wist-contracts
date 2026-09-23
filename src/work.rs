//! **工作授权快照**：网关授权（grant）、agentd 拉取（`WorkGrant`）并确认。
//!
//! ## 为什么是「快照」而不是「指令流」
//!
//! 常驻工作（`StandingWork`）表达的是**期望状态**：某个采集面应当持续执行什么。
//! 期望状态天然是幂等的（同一份重复拉取不产生副作用），断了网、重启了进程，
//! 再拉一次就回到期望 —— 不必让网关记住「上次推到哪条」。
//! 这跟 `PollControlCommands` 的长轮询指令流是两种东西：那条要求**不重放**，这条要求**可重拉**。
//!
//! 一次性工作（`OneShotWork`）是命令式的，但它也搭这份快照回来：
//! 决定「现在还该做吗」的是它的状态（未了结才出现在快照里），不是「推没推过」。
//!
//! ## 为什么在契约 crate
//!
//! 这是**两侧都要解析**的字节：网关写授权、agentd 读并执行。各写一份结构体，
//! 迟早出现「网关发了 `plan_version`、agentd 读的是 `version`」这种只能在真机联调时才发现的分叉。
//! 与 `DiscoveryAspectPolicySet` 同一个理由。
//!
//! ## 与本地保护暂停的区别
//!
//! agentd 自己也会暂停（如 `spool over limit`），那是**本地保护**：自动、临时、本机可见。
//! 这里下发的 `paused` 是**授权层状态**：人工、持久、跨重启。两者在观测里必须能分辨，
//! 不要合并成一个 paused —— 合并之后「谁把采集停了」就再也说不清。

use serde::{Deserialize, Serialize};

/// agentd → 网关：拉取工作授权快照的 envelope kind。
pub const POLL_WORK_KIND: &str = "poll_work";
/// agentd → 网关：确认收到工作的 envelope kind。
pub const ACK_WORK_KIND: &str = "ack_work";

/// 常驻工作的状态取值（对应模型 `StandingWork.status`）。
///
/// `superseded` 与 `revoked` 都不出现在 `WorkGrant.standing` 里：前者是「被新版本取代」，
/// 后者是「授权被撤」。两者都要留痕，所以还是要有状态而不是删行。
pub const STANDING_WORK_STATUSES: [&str; 4] = ["active", "paused", "superseded", "revoked"];

/// 一次性工作的状态取值（对应模型 `OneShotWork.status`）。
pub const ONE_SHOT_WORK_STATUSES: [&str; 9] = [
    "dispatched",
    "accepted",
    "running",
    "paused",
    "succeeded",
    "failed",
    "timed_out",
    "canceled",
    "expired",
];

/// 一次性工作的**终态**：到了这几个状态就了结了，不再出现在快照里。
pub const ONE_SHOT_TERMINAL_STATUSES: [&str; 5] =
    ["succeeded", "failed", "timed_out", "canceled", "expired"];

/// 工作类型：常驻（持续到被替换或撤回）与一次性（有期限与终态）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "state", domain = "Control", module = "Control.Agent.Work")]
#[serde(rename_all = "PascalCase")]
pub enum WorkKind {
    /// 常驻工作：按**采集面**授权，一个面一份。
    Standing,
    /// 一次性工作：按**动作**授权。
    OneShot,
}

/// 常驻工作：网关声明该 Agent 应当持续执行的工作。
///
/// 粒度是**采集面**（一个面 = 一份工作），不是 capability：一份 `collect_logs` 会裹住十几个面，
/// 那样「按面暂停 / 限流 / 审计」全都无从下手。面由模板的 `family_scope` 展开而来，
/// **不携带 capability** —— 该面由哪个采集器承接，由 `spec` 里的单元各自决定。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Control", module = "Control.Agent.Work")]
#[serde(deny_unknown_fields)]
pub struct StandingWork {
    pub work_id: String,
    pub agent_id: String,
    /// 采集面（`CollectionFamily`）。
    pub family: String,
    /// 工作参数：由采集目录的条目组合而成（**不是自由文本**），随 `plan_version` 整体替换。
    pub spec: String,
    /// 本工作按哪一版目录展开：目录换版**不追改**已授权工作（要跟新版得走新提案 + 审定）。
    pub catalog_version: i64,
    /// 生效依据：指向已批准的提案（人工直填 spec 时为空）。
    #[serde(default)]
    pub proposal_id: Option<String>,
    /// 期望版本：网关每次改动 +1；agentd 回报实际版本，与它比对即得漂移。
    pub plan_version: i64,
    pub effective_from: String,
    /// 见 [`STANDING_WORK_STATUSES`]。
    pub status: String,
    pub updated_by: String,
    pub updated_at: String,
}

/// 一次性工作：有计划开始时间与期限，有明确终态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Control", module = "Control.Agent.Work")]
#[serde(deny_unknown_fields)]
pub struct OneShotWork {
    pub work_id: String,
    pub agent_id: String,
    /// 动作面：upgrade / snapshot / exec / ...（执行载体是 Reporting 域的 ActionPlan）。
    pub action: String,
    pub spec: String,
    /// 计划开始时间：到点前不应执行（与「立即派发」区分开）。
    pub scheduled_at: String,
    /// 绝对截止：业务要求的时间点，**暂停也照走**（不由暂停顺延）。
    pub deadline_at: String,
    /// 执行预算（秒）：**只在实际执行时消耗**，暂停期间不计。
    pub timeout_seconds: i64,
    /// 可中断性：只有可中断的动作才允许运行中暂停。
    pub interruptible: bool,
    /// 见 [`ONE_SHOT_WORK_STATUSES`]。
    pub status: String,
    /// 当前暂停的起点（运行中暂停才有；恢复后清空）。
    #[serde(default)]
    pub paused_at: Option<String>,
    /// 累计暂停时长（秒）：用于审计与「预算未被暂停消耗」的核对。
    pub paused_total_seconds: i64,
    /// 步级断点：已完成步骤保留、`current_step` 在恢复时重做。
    #[serde(default)]
    pub current_step: Option<String>,
    #[serde(default)]
    pub completed_steps: Vec<String>,
    /// 已尝试次数（含恢复后的重做）。
    pub attempt: i64,
    pub issued_by: String,
    pub issued_at: String,
}

impl OneShotWork {
    /// 是否**未了结**（快照里只带未了结的活）。
    pub fn is_outstanding(&self) -> bool {
        !ONE_SHOT_TERMINAL_STATUSES.contains(&self.status.as_str())
    }

    /// 是否允许在运行期间暂停。
    ///
    /// 两条都要满足：动作声明了 `interruptible`，且此刻确实在做（`accepted`/`running`）。
    /// 不可中断的动作**拒绝**而不是「尽力暂停」—— 挂起半个升级进程比不暂停更危险。
    pub fn can_pause(&self) -> bool {
        self.interruptible && matches!(self.status.as_str(), "accepted" | "running")
    }
}

/// 工作授权快照：常驻工作的当前生效版本 + 未了结的一次性工作。
///
/// 幂等、可重复拉取；`sequence` 只用来让 agentd 判断「这份跟我手上的有没有变」，
/// **不承担「指令重放」的语义**（那是控制指令流的事）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Control", module = "Control.Agent.Work")]
#[serde(deny_unknown_fields)]
pub struct WorkGrant {
    pub agent_id: String,
    /// 每个面一条。
    #[serde(default)]
    pub standing: Vec<StandingWork>,
    #[serde(default)]
    pub one_shot: Vec<OneShotWork>,
    /// 授权序号（单调递增，每次授权/撤回/暂停/继续都 +1）。
    pub sequence: i64,
    pub granted_at: String,
}

/// 管理面授权或撤回工作的回执：一份工作一次。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Control", module = "Control.Agent.Work")]
#[serde(deny_unknown_fields)]
pub struct WorkReceipt {
    pub work_id: String,
    pub agent_id: String,
    pub work_kind: WorkKind,
    /// accepted | paused | resumed | revoked | rejected。
    pub status: String,
    pub plan_version: i64,
    pub created_at: String,
}

/// agentd → 网关：拉取工作授权快照。
///
/// 带 `last_seen_sequence`（与本机手上那份的序号），网关可以据此在没变化时短路；
/// 带 `wait_ms` 是为了允许将来的长轮询（现在是立即返回，字段先留着，免得改协议）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(
    kind = "message",
    role = "command",
    domain = "Control",
    module = "Control.AgentApp.FacingInterface"
)]
#[serde(deny_unknown_fields)]
pub struct PollWork {
    pub api_version: String,
    pub kind: String,
    pub agent_id: String,
    pub instance_id: String,
    pub last_seen_sequence: i64,
    pub wait_ms: i64,
    pub requested_at: String,
}

/// agentd → 网关：确认收到某份工作。
///
/// 常驻工作在 `plan_version` 变化后**也必须**确认：网关据此判断「期望的版本真到了吗」，
/// 一直没确认的就是漂移。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(
    kind = "message",
    role = "command",
    domain = "Control",
    module = "Control.AgentApp.FacingInterface"
)]
#[serde(deny_unknown_fields)]
pub struct AckWork {
    pub api_version: String,
    pub kind: String,
    pub agent_id: String,
    pub instance_id: String,
    pub work_id: String,
    pub plan_version: i64,
    pub acknowledged_at: String,
}

/// 网关对 [`AckWork`] 的回应。
///
/// 是**工作域的结构**而不是协议消息（与 [`WorkGrant`] 同类）：它描述的是
/// 「工作已被确认」这个领域事实，也要能被用例当成 outcome 引用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Control", module = "Control.Agent.Work")]
#[serde(deny_unknown_fields)]
pub struct WorkAccepted {
    pub work_id: String,
    /// accepted | stale | unknown。
    pub status: String,
    pub accepted_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn standing(work_id: &str, family: &str, plan_version: i64) -> StandingWork {
        StandingWork {
            work_id: work_id.to_string(),
            agent_id: "agent-1".to_string(),
            family: family.to_string(),
            spec: "unit-a,unit-b".to_string(),
            catalog_version: 1,
            proposal_id: None,
            plan_version,
            effective_from: "2026-09-23T00:00:00Z".to_string(),
            status: "active".to_string(),
            updated_by: "admin".to_string(),
            updated_at: "2026-09-23T00:00:00Z".to_string(),
        }
    }

    fn one_shot(status: &str, interruptible: bool) -> OneShotWork {
        OneShotWork {
            work_id: "work-1".to_string(),
            agent_id: "agent-1".to_string(),
            action: "upgrade".to_string(),
            spec: "0.1.4".to_string(),
            scheduled_at: "2026-09-23T00:00:00Z".to_string(),
            deadline_at: "2026-09-24T00:00:00Z".to_string(),
            timeout_seconds: 600,
            interruptible,
            status: status.to_string(),
            paused_at: None,
            paused_total_seconds: 0,
            current_step: None,
            completed_steps: vec![],
            attempt: 0,
            issued_by: "admin".to_string(),
            issued_at: "2026-09-23T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn terminal_one_shot_work_is_not_outstanding() {
        // 未了结的活才进快照：终态留在库里供审计，但不必再发给 agent。
        for status in ONE_SHOT_TERMINAL_STATUSES {
            assert!(!one_shot(status, true).is_outstanding(), "{status}");
        }
        for status in ["dispatched", "accepted", "running", "paused"] {
            assert!(one_shot(status, true).is_outstanding(), "{status}");
        }
    }

    #[test]
    fn only_interruptible_running_work_can_pause() {
        // 不可中断：拒绝，而不是「尽力暂停」。
        assert!(!one_shot("running", false).can_pause());
        // 可中断但没在做（还没到点 / 已经了结）：也没什么可暂停的。
        assert!(!one_shot("dispatched", true).can_pause());
        assert!(!one_shot("succeeded", true).can_pause());
        assert!(one_shot("accepted", true).can_pause());
        assert!(one_shot("running", true).can_pause());
    }

    #[test]
    fn work_kind_serializes_as_the_model_names() {
        assert_eq!(
            serde_json::to_string(&WorkKind::Standing).expect("serialize"),
            "\"Standing\""
        );
        assert_eq!(
            serde_json::to_string(&WorkKind::OneShot).expect("serialize"),
            "\"OneShot\""
        );
    }

    #[test]
    fn grant_round_trips_with_serde() {
        let grant = WorkGrant {
            agent_id: "agent-1".to_string(),
            standing: vec![standing("work-a", "LoginSession", 2)],
            one_shot: vec![one_shot("running", true)],
            sequence: 7,
            granted_at: "2026-09-23T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&grant).expect("serialize");
        let decoded: WorkGrant = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, grant);
    }

    #[test]
    fn grant_omits_absent_optional_fields_and_still_decodes() {
        // `proposal_id` / `paused_at` / `current_step` 缺省时必须能解析：
        // 手写 JSON（页面、联调）不该被迫填一堆 null。
        let json = r#"{"agent_id":"a","standing":[],"one_shot":[],"sequence":0,
                      "granted_at":"t"}"#;
        let grant: WorkGrant = serde_json::from_str(json).expect("deserialize");
        assert!(grant.standing.is_empty());
        assert!(grant.one_shot.is_empty());
    }

    #[test]
    fn rejects_unknown_fields() {
        // 两侧各自演进时，多出来的字段必须是响亮的错误：静默忽略会让
        // 「网关发了新字段、agentd 装作没看见」变成长期无声的语义分叉。
        let json = r#"{"agent_id":"a","standing":[],"one_shot":[],"sequence":0,
                      "granted_at":"t","extra":1}"#;
        assert!(serde_json::from_str::<WorkGrant>(json).is_err());
    }
}
