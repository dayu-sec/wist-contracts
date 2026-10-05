//! **数据面上送启用**：控制面决定「这个 Agent 现在该不该向数据面上送」，agentd 拉取并在内存生效。
//!
//! ## 为什么是「现算的下发」而不是一条配置
//!
//! 上送目标（`host:port`）是**控制面的事实**：只有网关知道数据面（warp-parse）在哪。
//! 把它写进 Agent 的静态配置意味着「改一次地址 = 重装所有 Agent」，而重装又会把配置
//! 重置回模板默认值 —— 那正是「授权前待命」被实现成 `kind = "file"` 之后反复踩到的坑。
//!
//! 所以这里只下发一个**派生结果**：网关每次被问到时，用「该 Agent 有没有生效的工作」
//! 加上管理面已设的「数据面上送地址」现算出来。没有新状态、没有推送通道，也不需要
//! 管理面多一个动作 —— 派活（写一条 standing work）下一个 poll 就变成 `enabled = true`，
//! 撤回即变回 `false`。
//!
//! ## 为什么另开端点，不塞进 [`crate::work::WorkGrant`]
//!
//! `WorkGrant` 带 `#[serde(deny_unknown_fields)]`。往里加字段会让**新网关 + 旧 agentd**
//! 直接解析失败：旧 agentd 连工作授权都收不到，会停在最后一次应用的工作上 —— 这是
//! 舰队级的静默停摆。独立端点对两个方向都安全：旧 agentd 从不调它；新 agentd 遇到
//! 旧网关得到 404，按「无下发」回落本机配置。
//!
//! ## 三方语义（agentd 侧生效规则，见 `agentd` 的 `effective_output`）
//!
//! * 未下发（未入网 / 旧网关 404 / 网络失败） → 用**本机** `[telemetry.logs.output]`；
//! * `enabled = false` → **待命**：不采、不写文件、不上送日志/指标（因此不会报错）；
//!   **但事实摘要仍上报** —— 它不是主机内容，而是让平台能推断「这台机器是什么」的最小
//!   元数据（进程列表 / 监听端口 / os / arch）。没有它，新装机器在网关侧一片空白，
//!   连「该派什么活」都定不下来（下发的目标指到哪，事实就发到哪，仅 tcp 能承载）；
//! * `enabled = true` → 若带 `host`/`port` 则**强制** `tcp(host, port)`（覆盖本机 `kind`），
//!   否则沿用本机 `kind`。
//!
//! 授权优先于本机：托管模式下目标与开关都由控制面说了算，本机配置只作 standalone 的兜底。

use serde::{Deserialize, Serialize};

/// Agent **实际生效**的采集输出状态（agent → 网关，自报）。
///
/// 与 [`AgentUplinkGrant`] 是一对：grant 说「控制面要它怎样」，这里说「它实际成了怎样」。
/// 为什么必须由 agent 上报：网关知道自己**下发**了什么，但不知道 agent **生效**成什么 ——
/// grant 可能还没拉到、可能被本机总闸拦住、可能目标连不上。这与
/// `AgentStatusReport::discovery_policy_version` 是同一类事实（「我发布了哪一版」≠「它生效了哪一版」）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Protocol")]
#[serde(deny_unknown_fields)]
pub struct AgentUplinkState {
    /// 生效输出的总闸：`false` = 待命（不读源、不写本地采集输出、不上送）。
    pub enabled: bool,
    /// 生效输出类型：`file` | `tcp`（grant 覆盖本机 `kind` 之后的**结果**）。
    pub kind: String,
    /// 生效目标（`tcp` 且有目标时）。`None` = 本地文件输出，或 tcp 但没有目标。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// 这份生效状态的来源：`grant`（控制面下发）| `local`（未下发，用本机配置）。
    ///
    /// 有了它，平台才能区分「控制面没启用」与「启用了但 agent 没听从」（后者才是故障）。
    pub source: String,
    /// 最近的出口写失败是否**尚未恢复**（细节在 agent 日志里：目标与原因）。
    pub output_write_failing: bool,
}

#[cfg(test)]
mod state_tests {
    use super::*;

    #[test]
    fn a_local_file_output_state_omits_the_target() {
        let state = AgentUplinkState {
            enabled: true,
            kind: "file".to_string(),
            target: None,
            source: "local".to_string(),
            output_write_failing: false,
        };
        let json = serde_json::to_string(&state).expect("encode");
        assert!(!json.contains("target"), "{json}");
        let back: AgentUplinkState = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, state);
    }

    #[test]
    fn a_grant_driven_tcp_state_carries_the_target() {
        let state = AgentUplinkState {
            enabled: true,
            kind: "tcp".to_string(),
            target: Some("10.0.1.9:9000".to_string()),
            source: "grant".to_string(),
            output_write_failing: true,
        };
        let json = serde_json::to_string(&state).expect("encode");
        let back: AgentUplinkState = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, state);
        // 与其它契约同口径：多出来的键要硬失败，别把分叉静默掉。
        assert!(
            serde_json::from_str::<AgentUplinkState>(
                r#"{"enabled":true,"kind":"tcp",
               "source":"local","output_write_failing":false,"extra":1}"#
            )
            .is_err()
        );
    }
}
