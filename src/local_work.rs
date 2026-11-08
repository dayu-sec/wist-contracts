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
/// 指标目标全部失败，都不会改变前三个字段。以前这些只在目标机的日志里，
/// 管理面看到的是一张“看起来正常”的卡。
///
/// 语义边界（避免把正常态报成故障）：
///   · `ok` = 本 tick **没有观测到采集失败**（采集任务已生效、没有报错）。
///     **不**意味着「这 tick 有新数据」—— 安静的日志文件本就是正常态；
///   · `degraded` = 还有内容在采，但部分任务 / 来源 / 目标失败；
///   · `failed` = 这个面没有可采内容，或全部任务都失败；
///   · `unknown` = 已授权但还没有可判定的观测（指标尚无目标 / 首轮未完成）；
///   · `paused` = 不是失败：工作被暂停，或数据面上送被关闸（待命）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Protocol")]
#[serde(deny_unknown_fields)]
pub struct AgentLocalCollection {
    /// 见 [`collection_status`]。
    pub status: String,
    /// 本机时钟：得出这个结论的那一轮采集。
    pub observed_at: String,
    /// 人看的说明：失败原因原样带上；`ok` 时为空。
    #[serde(default)]
    pub detail: String,
}

/// [`AgentLocalCollection::status`] 的取值（闭集）。
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
                    status: collection_status::OK.to_string(),
                    observed_at: "2026-09-27T00:00:00Z".to_string(),
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
            status: collection_status::DEGRADED.to_string(),
            observed_at: "2026-09-27T00:00:00Z".to_string(),
            detail: "1/2 个采集任务失败：/var/log/app.log 不存在".to_string(),
        };
        let json = serde_json::to_string(&collection).expect("encode");
        let back: AgentLocalCollection = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, collection);

        let mutated = json.replacen('{', "{\"extra\":1,", 1);
        assert!(serde_json::from_str::<AgentLocalCollection>(&mutated).is_err());
    }

    #[test]
    fn collection_detail_defaults_to_empty_and_status_stays_open() {
        // `detail` 是加性字段：旧 agent 不写也能解。
        let json = r#"{"status":"degraded","observed_at":"2026-09-27T00:00:00Z"}"#;
        let collection: AgentLocalCollection = serde_json::from_str(json).expect("decode");
        assert_eq!(collection.detail, "");

        // status 是**开放字符串**（不枚举反序列化）：未来新增状态不能让旧网关整份报文解码失败，
        // 页面按“认不出就原样显示”兜底。
        let json = r#"{"status":"future_state","observed_at":"t"}"#;
        let collection: AgentLocalCollection = serde_json::from_str(json).expect("open status");
        assert_eq!(collection.status, "future_state");
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
