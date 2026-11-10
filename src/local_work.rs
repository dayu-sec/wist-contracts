//! agentd 上报的**本机工作内容视图**（`state/work.json` 的同形子集）。
//!
//! 为什么要有它：网关知道自己**授权**了什么（`StandingWork.spec`），但那不等于「agent 真的在采
//! 哪些文件」—— 本机配置里手工加的输入、工作被暂停、某条来源今天接不接得了，都只有 agent 自己
//! 知道。`state/work.json` 就是这份本机事实，但它只落在目标机上；把它的**必要子集**随状态上报
//! 带上来，页面上才能回答「这台机器现在到底在干什么」。
//!
//! 刻意**不带**工作内容（`units`）：那是网关发下去的，网关自己有 —— 本机再抄一份只会诱使人拿它
//! 当第二份真相（与 `state_store::work` 的取舍一致）。

use serde::{Deserialize, Serialize};

/// 本机一条**采集任务**：一个任务 id 盯一个路径。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Protocol")]
#[serde(deny_unknown_fields)]
pub struct AgentLocalTask {
    /// 任务 id：同时是本机 `state/logs/file_inputs/<input_id>/` 与 spool 的目录名。
    pub input_id: String,
    /// 盯的文件路径（今天只实现显式绝对路径）。
    pub path: String,
    /// `head` | `tail`：起读位置。授权采集一律 `tail`（不重放历史）。
    pub startup_position: String,
}

/// 一个采集面的**采集结果**（agentd 本机观测，随工作视图上报）。
///
/// 为什么需要它：`status` / `acknowledged_version` / `tasks` 只能回答「工作收到没、
/// 准备采哪些文件」，回答不了「**到底采成没采成**」—— 文件不存在、没权限、导出器没装、
/// 指标目标全部失败，都不会改变前三个字段。
///
/// **模型是多字段、正变的**（不要把它们折回单一状态）：
///   · [`prerequisites`](Self::prerequisites)：前置条件是否具备（工具 / 依赖 / 来源 / 目标）；
///   · [`result`](Self::result)：执行结果（成功 / 部分 / 失败 / 取消 / 待定）；
///   · [`reasons`](Self::reasons)：为什么不是成功（结构化，不参与判定）。
///
/// 判定规则见 wist-agentd `docs/design/collection-status.md`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Protocol")]
#[serde(deny_unknown_fields)]
pub struct AgentLocalCollection {
    /// 前置条件：见 [`collection_prerequisites`]。
    #[serde(default)]
    pub prerequisites: String,
    /// 执行结果：见 [`collection_result`]。
    #[serde(default)]
    pub result: String,
    /// 原因（有序，页面按顺序展示）。
    #[serde(default)]
    pub reasons: Vec<CollectionReason>,
    /// 本机时钟：得出这个结论的那一轮采集。
    pub observed_at: String,

    /// **兼容字段（滚动升级期）**：旧 agentd 只报 `status` / `detail`。
    /// 新消费方以 `result` 为准；旧消费方继续使用本字段。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub status: String,
    /// 兼容字段：旧版人看说明。新模型用 [`reasons`](Self::reasons)。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

/// 一条采集原因：不参与状态判定，只回答「为什么」。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Protocol")]
#[serde(deny_unknown_fields)]
pub struct CollectionReason {
    /// 见 [`collection_reason_kind`]。
    pub kind: String,
    /// 具体对象：工具名 / 输入 id / 指标域。
    #[serde(default)]
    pub subject: String,
    /// 人看的说明。
    #[serde(default)]
    pub detail: String,
}

/// [`AgentLocalCollection::prerequisites`] 的取值（闭集）。
pub mod collection_prerequisites {
    /// 工具、依赖、来源、目标都已具备。
    pub const READY: &str = "ready";
    /// 至少一个必备条件缺失；具体原因见 `reasons`。
    pub const BLOCKED: &str = "blocked";

    pub const ALL: &[&str] = &[READY, BLOCKED];
}

/// [`AgentLocalCollection::result`] 的取值（闭集）。
pub mod collection_result {
    /// 尝试过的单元全部成功，且没有软缺陷。
    pub const SUCCESS: &str = "success";
    /// 有内容采到，但部分单元失败/被阻塞，或有软缺陷（如截断）。
    pub const PARTIAL: &str = "partial";
    /// 尝试过但全部失败，或结构性错误（参数解析失败 / 零单元）。
    pub const FAILURE: &str = "failure";
    /// 未执行：工作暂停 / 上送关闸 / 前置条件不满足。
    pub const CANCELLED: &str = "cancelled";
    /// 还没有可判定的观测（首轮未完成）；不是错误，也不是取消。
    pub const PENDING: &str = "pending";

    pub const ALL: &[&str] = &[SUCCESS, PARTIAL, FAILURE, CANCELLED, PENDING];
}

/// [`CollectionReason::kind`] 的取值（闭集）。
pub mod collection_reason_kind {
    pub const MISSING_TOOL: &str = "missing_tool";
    pub const MISSING_DEPENDENCY: &str = "missing_dependency";
    pub const SOURCE_NOT_READY: &str = "source_not_ready";
    pub const TARGET_NOT_READY: &str = "target_not_ready";
    pub const PERMISSION: &str = "permission";
    pub const MISSING_INPUT: &str = "missing_input";
    pub const RUN_FAILED: &str = "run_failed";
    pub const UNSUPPORTED: &str = "unsupported";
    pub const TRUNCATED: &str = "truncated";
    pub const INVALID_SPEC: &str = "invalid_spec";
    pub const NO_UNITS: &str = "no_units";
    pub const NO_CONTENT: &str = "no_content";
    pub const INCOMPLETE_DATA: &str = "incomplete_data";
    pub const PAUSED: &str = "paused";
    pub const STANDBY: &str = "standby";
    pub const METRIC_PARTIAL: &str = "metric_partial";

    pub const ALL: &[&str] = &[
        MISSING_TOOL,
        MISSING_DEPENDENCY,
        SOURCE_NOT_READY,
        TARGET_NOT_READY,
        PERMISSION,
        MISSING_INPUT,
        RUN_FAILED,
        UNSUPPORTED,
        TRUNCATED,
        INVALID_SPEC,
        NO_UNITS,
        NO_CONTENT,
        INCOMPLETE_DATA,
        PAUSED,
        STANDBY,
        METRIC_PARTIAL,
    ];
}

/// **兼容字段（滚动升级期）**：旧 [`AgentLocalCollection::status`] 的取值（闭集）。
///
/// 新代码不得再产生新的取值；新模型见 [`collection_result`] 与 [`collection_prerequisites`]。
pub mod collection_status {
    pub const OK: &str = "ok";
    pub const DEGRADED: &str = "degraded";
    pub const FAILED: &str = "failed";
    pub const UNKNOWN: &str = "unknown";
    pub const PAUSED: &str = "paused";

    pub const ALL: &[&str] = &[OK, DEGRADED, FAILED, UNKNOWN, PAUSED];
}

/// 一份**常驻工作**在本机的样子。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Protocol")]
#[serde(deny_unknown_fields)]
pub struct AgentLocalStandingWork {
    pub work_id: String,
    /// 采集面（`CollectionFamily`）。
    pub family: String,
    /// `active` | `paused`（被撤回 / 被取代的不算「我手里的工作」）。
    pub status: String,
    /// 网关的期望版本。
    pub plan_version: i64,
    /// 我回报过的版本；`None` = 还没回报（网关那边看到的就是漂移）。
    #[serde(default)]
    pub acknowledged_version: Option<i64>,
    pub effective_from: String,
    /// 折算成的本机采集任务（指标类工作没有任务）。
    #[serde(default)]
    pub tasks: Vec<AgentLocalTask>,
    /// 本机对这份工作的**采集结果**。
    ///
    /// `None` = 旧版 agentd 没报（或还没跑过一轮采集）—— 与 `unknown` 不同：
    /// 前者是「本机根本不说」，后者是「说了，但还没有可判定的观测」。
    ///
    /// 部署顺序：**gateway 先于 agentd** —— 父结构是 `deny_unknown_fields`，
    /// 旧网关遇到新字段会直接解析失败（与本文件其他新增字段同一条规则）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection: Option<AgentLocalCollection>,
}

/// 一件**一次性工作**在本机的样子。
///
/// 两个状态轴分开：`status` 是**网关侧**的派发状态（dispatched / accepted / …），`execution` 是
/// **本机执行状态**（现在一律 `unexecuted`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Protocol")]
#[serde(deny_unknown_fields)]
pub struct AgentLocalOneShotWork {
    pub work_id: String,
    /// 动作面：upgrade / snapshot / exec / …。
    pub action: String,
    pub status: String,
    /// `unexecuted`（本机尚未执行）。
    pub execution: String,
    pub scheduled_at: String,
    pub deadline_at: String,
    pub timeout_seconds: i64,
}

/// agentd 上报的**本机工作视图**。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Protocol")]
#[serde(deny_unknown_fields)]
pub struct AgentLocalWork {
    /// 这份视图是什么时候算出来的（**本机时钟**）。
    pub recorded_at: String,
    /// 溯源：本视图对应网关的授权序号。**不是授权副本** —— 只用来把本机视图与网关那一版对上号。
    pub gateway_sequence: i64,
    /// 手里生效或暂停的常驻工作。
    #[serde(default)]
    pub standing: Vec<AgentLocalStandingWork>,
    /// 手里未了结的一次性工作。
    #[serde(default)]
    pub one_shot: Vec<AgentLocalOneShotWork>,
    /// 本机配置里手工加的日志输入（运维逃生舱）：**不来自任何采集面**，网关无从得知。
    #[serde(default)]
    pub local_inputs: Vec<AgentLocalTask>,
    /// 汇总：指标上送周期（秒）；`None` = 不上送。
    #[serde(default)]
    pub metrics_interval_seconds: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minimal_report_decodes_with_empty_collections() {
        // 只报「我算过一份视图」时，四类列表与周期都不写也应能解析。
        let json = r#"{"recorded_at":"2026-09-27T00:00:00Z","gateway_sequence":4}"#;
        let work: AgentLocalWork = serde_json::from_str(json).expect("decode");
        assert!(work.standing.is_empty());
        assert!(work.one_shot.is_empty());
        assert!(work.local_inputs.is_empty());
        assert_eq!(work.metrics_interval_seconds, None);
    }

    #[test]
    fn a_full_report_round_trips_and_rejects_unknown_fields() {
        let work = AgentLocalWork {
            recorded_at: "2026-09-27T00:00:00Z".to_string(),
            gateway_sequence: 9,
            standing: vec![AgentLocalStandingWork {
                work_id: "work-1".to_string(),
                family: "ServiceLifecycle".to_string(),
                status: "active".to_string(),
                plan_version: 2,
                acknowledged_version: Some(2),
                effective_from: "2026-09-27T00:00:00Z".to_string(),
                tasks: vec![AgentLocalTask {
                    input_id: "app".to_string(),
                    path: "/var/log/app.log".to_string(),
                    startup_position: "tail".to_string(),
                }],
                collection: Some(AgentLocalCollection {
                    prerequisites: collection_prerequisites::READY.to_string(),
                    result: collection_result::SUCCESS.to_string(),
                    reasons: Vec::new(),
                    observed_at: "2026-09-27T00:00:00Z".to_string(),
                    status: collection_status::OK.to_string(),
                    detail: String::new(),
                }),
            }],
            one_shot: vec![AgentLocalOneShotWork {
                work_id: "work-2".to_string(),
                action: "upgrade".to_string(),
                status: "dispatched".to_string(),
                execution: "unexecuted".to_string(),
                scheduled_at: "2026-09-27T00:00:00Z".to_string(),
                deadline_at: "2026-09-28T00:00:00Z".to_string(),
                timeout_seconds: 600,
            }],
            local_inputs: Vec::new(),
            metrics_interval_seconds: Some(15),
        };
        let json = serde_json::to_string(&work).expect("encode");
        let back: AgentLocalWork = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, work);

        let mutated = json.replacen('{', "{\"extra\":1,", 1);
        assert!(serde_json::from_str::<AgentLocalWork>(&mutated).is_err());
    }

    #[test]
    fn a_standing_work_without_collection_stays_decodable_and_omits_the_key() {
        // 旧 agentd（或还没跑过采集）不带这个字段：必须照常解析，且编码时不凭空编一个。
        let json = r#"{"work_id":"w","family":"LoginSession","status":"active",
                       "plan_version":1,"acknowledged_version":1,
                       "effective_from":"2026-09-27T00:00:00Z"}"#;
        let work: AgentLocalStandingWork = serde_json::from_str(json).expect("decode");
        assert!(work.tasks.is_empty());
        assert!(work.collection.is_none());

        let encoded = serde_json::to_string(&work).expect("encode");
        assert!(!encoded.contains("collection"), "{encoded}");

        // `null` 与缺键同义（旧 agentd / 未观测）：都不能让解码失败。
        let with_null = r#"{"work_id":"w","family":"LoginSession","status":"active",
                            "plan_version":1,"acknowledged_version":1,
                            "effective_from":"2026-09-27T00:00:00Z","collection":null}"#;
        let work: AgentLocalStandingWork = serde_json::from_str(with_null).expect("null decodes");
        assert!(work.collection.is_none());
    }

    #[test]
    fn collection_results_round_trip_and_reject_unknown_fields() {
        let collection = AgentLocalCollection {
            prerequisites: collection_prerequisites::READY.to_string(),
            result: collection_result::PARTIAL.to_string(),
            reasons: vec![CollectionReason {
                kind: collection_reason_kind::MISSING_INPUT.to_string(),
                subject: "/var/log/app.log".to_string(),
                detail: "source path does not exist".to_string(),
            }],
            observed_at: "2026-09-27T00:00:00Z".to_string(),
            status: collection_status::DEGRADED.to_string(),
            detail: "1/2 个采集任务失败：/var/log/app.log 不存在".to_string(),
        };
        let json = serde_json::to_string(&collection).expect("encode");
        let back: AgentLocalCollection = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, collection);

        let mutated = json.replacen('{', "{\"extra\":1,", 1);
        assert!(serde_json::from_str::<AgentLocalCollection>(&mutated).is_err());
    }

    #[test]
    fn legacy_collection_reports_still_decode() {
        // 旧 agentd 只报 `status` / `detail`：新字段全部缺省，不能因此拒收整份上报。
        let json = r#"{"status":"degraded","observed_at":"2026-09-27T00:00:00Z"}"#;
        let collection: AgentLocalCollection = serde_json::from_str(json).expect("decode");
        assert_eq!(collection.detail, "");
        assert_eq!(collection.status, "degraded");
        assert_eq!(collection.prerequisites, "");
        assert_eq!(collection.result, "");
        assert!(collection.reasons.is_empty());

        // status 是**开放字符串**：未来新增取值不能让旧网关整份报文解码失败。
        let json = r#"{"status":"future_state","observed_at":"t"}"#;
        let collection: AgentLocalCollection = serde_json::from_str(json).expect("open status");
        assert_eq!(collection.status, "future_state");
    }

    #[test]
    fn new_collection_reports_decode_without_the_legacy_status() {
        // 新 agentd 只报三字段；旧 `status` / `detail` 可缺。
        let json = r#"{
            "prerequisites":"blocked",
            "result":"cancelled",
            "reasons":[{"kind":"missing_tool","subject":"smartctl",
                        "detail":"工具缺失：/usr/sbin/smartctl"}],
            "observed_at":"2026-11-10T00:00:00Z"
        }"#;
        let collection: AgentLocalCollection = serde_json::from_str(json).expect("decode");
        assert_eq!(collection.prerequisites, collection_prerequisites::BLOCKED);
        assert_eq!(collection.result, collection_result::CANCELLED);
        assert_eq!(collection.reasons.len(), 1);
        assert_eq!(
            collection.reasons[0].kind,
            collection_reason_kind::MISSING_TOOL
        );
        assert!(collection.status.is_empty());
        assert!(collection.detail.is_empty());
    }

    #[test]
    fn new_collection_constants_are_closed_and_unique() {
        for (name, all) in [
            ("prerequisites", collection_prerequisites::ALL),
            ("result", collection_result::ALL),
            ("reason_kind", collection_reason_kind::ALL),
        ] {
            let mut sorted = all.to_vec();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), all.len(), "{name} 常量不得重复");
            for value in all {
                assert!(!value.is_empty(), "{name} 不得有空值");
                assert!(
                    value
                        .chars()
                        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_'),
                    "{name} 必须是 snake_case：{value}"
                );
            }
        }
        for expected in ["ready", "blocked"] {
            assert!(collection_prerequisites::ALL.contains(&expected));
        }
        for expected in ["success", "partial", "failure", "cancelled", "pending"] {
            assert!(collection_result::ALL.contains(&expected));
        }
    }

    #[test]
    fn nested_collection_rejects_unknown_fields() {
        // 父结构与子结构都是 deny_unknown_fields：两侧各自演进时，多出来的键要响亮地失败。
        let json = r#"{"work_id":"w","family":"LoginSession","status":"active",
                       "plan_version":1,"acknowledged_version":1,
                       "effective_from":"2026-09-27T00:00:00Z",
                       "collection":{"status":"ok","observed_at":"t","extra":1}}"#;
        assert!(serde_json::from_str::<AgentLocalStandingWork>(json).is_err());
    }

    #[test]
    fn collection_status_constants_cover_the_closed_set() {
        let mut all = collection_status::ALL.to_vec();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), collection_status::ALL.len(), "状态常量不得重复");
        for expected in ["ok", "degraded", "failed", "unknown", "paused"] {
            assert!(all.contains(&expected), "缺少状态 {expected}");
        }
    }
}
