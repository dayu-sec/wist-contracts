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

/// agentd → 网关：拉取数据面上送启用的 envelope kind。
pub const POLL_AGENT_UPLINK_KIND: &str = "poll_agent_uplink";

/// agentd → 网关：拉取数据面上送启用。
///
/// 与 [`crate::work::PollWork`] 同形（同一套 agent 凭据、同一份实例标识），因为它是
/// 同一类「拉期望状态」的动作；只是期望状态的内容不同。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(
    kind = "message",
    role = "command",
    domain = "Control",
    module = "Control.AgentApp.FacingInterface"
)]
#[serde(deny_unknown_fields)]
pub struct PollAgentUplink {
    pub api_version: String,
    pub kind: String,
    pub agent_id: String,
    pub instance_id: String,
    pub requested_at: String,
}

/// 网关 → agentd：数据面上送的当前期望状态。
///
/// 与 [`crate::work::WorkGrant`] 同类：描述的是「这个 Agent 的上送现在应当是什么样」这个
/// 领域事实（幂等、可重复拉取），而不是一次协议动作。它**派生自工作授权**，所以与
/// `WorkGrant` 同属 `Control.Agent.Work`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Control", module = "Control.Agent.Work")]
#[serde(deny_unknown_fields)]
pub struct AgentUplinkGrant {
    /// 是否启用数据面上送的**主机内容**（日志 / 指标）。`false` = 待命：不采集、不上送日志与
    /// 指标。**但事实摘要不受它管** —— 待命期仍上报进程列表等推断元数据（见模块文档）。
    pub enabled: bool,
    /// 数据面地址。`enabled = true` 时给出即**覆盖**本机 `tcp.addr`；
    /// 缺省表示「沿用本机配置的目标」（本机 `kind = "file"` 时就仍是本地文件）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    pub granted_at: String,
}

impl AgentUplinkGrant {
    /// 待命：明确关掉。网关在「没有生效工作」或「管理面还没设上送地址」时下发它。
    pub fn standby(granted_at: String) -> Self {
        Self {
            enabled: false,
            host: None,
            port: None,
            granted_at,
        }
    }

    /// 启用：带上目标。`host`/`port` 由管理面的「数据面上送地址」给。
    pub fn enabled_at(host: String, port: u16, granted_at: String) -> Self {
        Self {
            enabled: true,
            host: Some(host),
            port: Some(port),
            granted_at,
        }
    }

    /// 要覆盖的本机目标（`enabled` 且绑定齐了 `host`/`port` 才有）。
    ///
    /// `host` 会 `trim`：空白主机（`" "`）不是目标 —— 把它当目标只会拼出连不上的
    /// `" :9000"`，而「拼出一个错地址」比「不覆盖、回落本机」难查得多。
    pub fn target(&self) -> Option<(&str, u16)> {
        match (self.enabled, self.host.as_deref(), self.port) {
            (true, Some(host), Some(port)) => {
                let host = host.trim();
                (!host.is_empty()).then_some((host, port))
            }
            _ => None,
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standby_round_trips_and_omits_absent_target() {
        // 待命帧不该把空目标序列化出去：`host`/`port` 缺失就是「沿用本机」的判据。
        let grant = AgentUplinkGrant::standby("2026-09-26T00:00:00Z".to_string());
        let json = serde_json::to_string(&grant).expect("encode");
        assert!(!json.contains("host"), "{json}");
        assert!(!json.contains("port"), "{json}");
        let back: AgentUplinkGrant = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, grant);
        assert_eq!(back.target(), None);
    }

    #[test]
    fn an_enabled_grant_carries_the_target_to_override() {
        let grant = AgentUplinkGrant::enabled_at(
            "c-001.gateway.example".to_string(),
            9000,
            "2026-09-26T00:00:00Z".to_string(),
        );
        assert_eq!(grant.target(), Some(("c-001.gateway.example", 9000)));
        let json = serde_json::to_string(&grant).expect("encode");
        let back: AgentUplinkGrant = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, grant);
    }

    #[test]
    fn an_enabled_grant_without_a_target_falls_back_to_the_local_kind() {
        // 允许「只开不指目标」：本机 kind 说了算。`target()` 必须为 None 而不是拼出空地址。
        let grant = AgentUplinkGrant {
            enabled: true,
            host: None,
            port: None,
            granted_at: "t".to_string(),
        };
        assert_eq!(grant.target(), None);

        // 空 host 也不能被当成目标：那只会拼出连不上的 ":9000"。
        let blank = AgentUplinkGrant {
            enabled: true,
            host: Some(String::new()),
            port: Some(9000),
            granted_at: "t".to_string(),
        };
        assert_eq!(blank.target(), None);

        // 只有空白的 host 同理（网关写入时会 trim，这里是第二道）。
        let whitespace = AgentUplinkGrant {
            enabled: true,
            host: Some("   ".to_string()),
            port: Some(9000),
            granted_at: "t".to_string(),
        };
        assert_eq!(whitespace.target(), None);

        // 前后带空白的真实主机要被裁成可用目标，而不是被判为无效。
        let padded = AgentUplinkGrant {
            enabled: true,
            host: Some(" gw.example ".to_string()),
            port: Some(9000),
            granted_at: "t".to_string(),
        };
        assert_eq!(padded.target(), Some(("gw.example", 9000)));

        // 有 host 但缺 port 也不算目标：缺一半就只能拼出 `host:` 这种残地址。
        let host_only = AgentUplinkGrant {
            enabled: true,
            host: Some("gw.example".to_string()),
            port: None,
            granted_at: "t".to_string(),
        };
        assert_eq!(host_only.target(), None);

        // 未启用的 grant 即使目标齐全也不构成覆盖。
        let standby_with_target = AgentUplinkGrant {
            enabled: false,
            host: Some("gw.example".to_string()),
            port: Some(9000),
            granted_at: "t".to_string(),
        };
        assert_eq!(standby_with_target.target(), None);
    }

    #[test]
    fn a_newer_gateway_field_is_rejected_rather_than_ignored() {
        // 契约是两侧都解析的字节：多出来的键必须硬失败，否则「网关发了 agentd 没读的字段」
        // 会变成静默分叉（与 work.rs 同一约定）。
        let json = r#"{"enabled":true,"granted_at":"t","extra":1}"#;
        assert!(serde_json::from_str::<AgentUplinkGrant>(json).is_err());
    }

    #[test]
    fn poll_round_trips_and_rejects_unknown_fields() {
        let poll = PollAgentUplink {
            api_version: crate::API_VERSION_V1.to_string(),
            kind: POLL_AGENT_UPLINK_KIND.to_string(),
            agent_id: "agent-1".to_string(),
            instance_id: "inst-1".to_string(),
            requested_at: "2026-09-26T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&poll).expect("encode");
        let back: PollAgentUplink = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, poll);

        let bad = r#"{"api_version":"v1","kind":"poll_agent_uplink","agent_id":"a",
                      "instance_id":"i","requested_at":"t","extra":1}"#;
        assert!(serde_json::from_str::<PollAgentUplink>(bad).is_err());
    }
}
