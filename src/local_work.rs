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
}
