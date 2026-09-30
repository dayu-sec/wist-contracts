//! 发现方向的**策略表**：平台策展，网关装载并下发给 agentd。
//!
//! ## 为什么要下发，而不是各自写死
//!
//! 各方向的观测周期是**平台级**取舍（采得密是为了少漏检，报得疏是为了不噪声），
//! 不该在每台机器的二进制里各写一份。写死的代价已经在代码里显形过：
//! 探针的 `refresh_interval()` 里注释着「值取自模型 DiscoveryAspectPolicy.default_interval_seconds」，
//! 而那份模型值**没有任何通路进代码** —— 于是模型与代码成了两份可以静默漂移的抄本。
//! 现在：值放 `wist-knowledge/aspect-policies.toml`（与用途规则表同样的策展数据），
//! 网关装载并校验后下发，agentd 的应用逻辑只认这一份。
//!
//! ## 为什么在契约 crate
//!
//! 这是**两侧都要解析**的字节：网关装载校验、agentd 拉取应用。各写一份结构体就会出现
//! 「网关发了字段 A、agentd 读的是字段 B」这种只能在真机联调时才发现的问题。
//!
//! ## 未配置时的语义
//!
//! 策略表是**可选**的：网关没配表就**不提供**这个端点（agentd 继续用自己的内建默认值），
//! 而不是发一份空表 —— 空表会让「平台没发布策略」与「发布了空策略」变得无法区分。

use serde::{Deserialize, Serialize};

/// 发现方向的闭集（与模型 `Discovery.Probe.DiscoveryAspect` 一致）。
///
/// 用字符串而不是 Rust 枚举承载：它同时是**探针名**（`DiscoveryProbe::name()` 返回的
/// `"host"` / `"process"` …），agentd 侧直接拿它查表，不必再维护一层映射。
pub const DISCOVERY_ASPECTS: [&str; 7] = [
    "host",
    "network",
    "process",
    "endpoint",
    "container",
    "k8s",
    "package",
];

/// 平台闭集。
pub const DISCOVERY_PLATFORMS: [&str; 2] = ["macos", "linux"];

/// 单个发现方向的策略。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Probe")]
#[serde(deny_unknown_fields)]
pub struct DiscoveryAspectPolicy {
    /// 方向名（见 [`DISCOVERY_ASPECTS`]）。
    pub aspect: String,
    /// **观测**周期（秒）：多久采一次。
    pub default_interval_seconds: i64,
    /// 下限：网关侧改这个方向时不允许低于它（采得太密会把被管机器拖住）。
    pub min_interval_seconds: i64,
    /// 上限：同理，采得太疏就失了观测意义。
    pub max_interval_seconds: i64,
    /// 基线面：回答「我是什么」，是自识别与派活的底座 —— 不可关。
    pub baseline: bool,
    /// 未显式声明时的默认开关。
    pub enabled_by_default: bool,
    /// 适用平台（见 [`DISCOVERY_PLATFORMS`]）。
    #[serde(default)]
    pub platforms: Vec<String>,
    /// 产出的事实（供人理解「为什么要这个面」），如 `os/arch/核数/内存/磁盘/GPU`。
    #[serde(default)]
    pub yields: String,
}

impl DiscoveryAspectPolicy {
    /// 该方向是否适用于某平台。
    pub fn supports(&self, platform: &str) -> bool {
        self.platforms.iter().any(|item| item == platform)
    }
}

/// 一版策略表：版本号 + 发布时间 + 各方向的策略。
///
/// 形状刻意与 `PurposeRuleSet` 对齐（版本 + 发布时间 + 条目列表），因为它是**同一种东西**：
/// 平台策展、按版本发布、改内容必须同时 bump 版本（否则两侧无法判断谁过期了）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Discovery", module = "Discovery.Probe")]
#[serde(deny_unknown_fields)]
pub struct DiscoveryAspectPolicySet {
    pub policy_version: i64,
    pub published_at: String,
    #[serde(default)]
    pub policies: Vec<DiscoveryAspectPolicy>,
}

impl DiscoveryAspectPolicySet {
    pub fn new(
        policy_version: i64,
        published_at: String,
        policies: Vec<DiscoveryAspectPolicy>,
    ) -> Self {
        Self {
            policy_version,
            published_at,
            policies,
        }
    }

    /// 取某个方向的策略；表里没有该方向返回 `None`（调用方回退到自己的默认值）。
    pub fn for_aspect(&self, aspect: &str) -> Option<&DiscoveryAspectPolicy> {
        self.policies.iter().find(|policy| policy.aspect == aspect)
    }

    /// 取某个方向的观测周期（秒）。
    pub fn interval_seconds_for(&self, aspect: &str) -> Option<i64> {
        self.for_aspect(aspect)
            .map(|policy| policy.default_interval_seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(aspect: &str, interval: i64) -> DiscoveryAspectPolicy {
        DiscoveryAspectPolicy {
            aspect: aspect.to_string(),
            default_interval_seconds: interval,
            min_interval_seconds: 60,
            max_interval_seconds: 3600,
            baseline: false,
            enabled_by_default: true,
            platforms: vec!["macos".to_string()],
            yields: String::new(),
        }
    }

    #[test]
    fn looks_up_a_policy_by_aspect_name() {
        let set = DiscoveryAspectPolicySet::new(
            3,
            "2026-09-22T00:00:00Z".to_string(),
            vec![policy("host", 900), policy("process", 300)],
        );

        assert_eq!(set.interval_seconds_for("process"), Some(300));
        assert_eq!(set.for_aspect("host").map(|p| p.baseline), Some(false));
        // 表里没有的方向：回退由调用方决定，这里如实返回 None。
        assert_eq!(set.interval_seconds_for("package"), None);
    }

    #[test]
    fn platform_membership_is_exact() {
        let mut policy = policy("endpoint", 300);
        policy.platforms = vec!["linux".to_string()];

        assert!(policy.supports("linux"));
        assert!(!policy.supports("macos"));
        // 不做前缀匹配：`linux` 不该匹配 `linux2`。
        assert!(!policy.supports("linux2"));
    }

    #[test]
    fn round_trips_with_serde() {
        let set = DiscoveryAspectPolicySet::new(
            1,
            "2026-09-22T00:00:00Z".to_string(),
            vec![policy("host", 900)],
        );
        let json = serde_json::to_string(&set).expect("serialize");
        let decoded: DiscoveryAspectPolicySet = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(decoded, set);
    }

    #[test]
    fn rejects_unknown_fields() {
        // 两侧各自演进时，多出来的字段必须是**响亮的错误**，而不是静默忽略 ——
        // 静默忽略会让「网关发了新字段、agentd 装作没看见」变成长期无声的语义分叉。
        let json = r#"{"policy_version":1,"published_at":"x","policies":[],"extra":true}"#;
        assert!(serde_json::from_str::<DiscoveryAspectPolicySet>(json).is_err());
    }

    #[test]
    fn an_empty_table_has_no_policy_for_any_aspect() {
        // 未配置的表与「配了一个空表」在这里是同形的：调用方据此回退到自己的内建默认值。
        let set = DiscoveryAspectPolicySet::new(0, "2026-09-27T00:00:00Z".to_string(), Vec::new());
        for aspect in DISCOVERY_ASPECTS {
            assert_eq!(set.interval_seconds_for(aspect), None, "{aspect}");
            assert!(set.for_aspect(aspect).is_none(), "{aspect}");
        }
    }

    #[test]
    fn omitted_optional_fields_decode_to_stable_defaults() {
        // 手写策略表（页面、联调）不写 platforms/yields 时应能解析，而不是报缺字段。
        let set: DiscoveryAspectPolicySet =
            serde_json::from_str(r#"{"policy_version":1,"published_at":"t"}"#).expect("decode");
        assert!(set.policies.is_empty());

        let policy: DiscoveryAspectPolicy =
            serde_json::from_str(
                r#"{"aspect":"host","default_interval_seconds":900,"min_interval_seconds":60,"max_interval_seconds":3600,"baseline":true,"enabled_by_default":true}"#,
            )
            .expect("decode");
        assert!(policy.platforms.is_empty());
        assert_eq!(policy.yields, "");
    }
}
