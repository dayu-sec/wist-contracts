//! `CapabilityReport` contract types.

use serde::{Deserialize, Serialize};

use crate::SCHEMA_VERSION_V1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
#[serde(deny_unknown_fields)]
pub struct CapabilityReport {
    pub schema_version: String,
    pub agent_id: String,
    pub instance_id: String,
    pub reported_at: String,
    pub exec: ExecCapabilities,
    pub metrics: MetricsCapabilities,
    pub logs: Option<LogsCapabilities>,
    pub upgrade: UpgradeCapabilities,
    pub limits: CapabilityLimits,
}

#[derive(Debug, Clone, PartialEq, Eq, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
pub struct CapabilityReportSections {
    pub agent_id: String,
    pub instance_id: String,
    pub reported_at: String,
    pub exec: ExecCapabilities,
    pub metrics: MetricsCapabilities,
    pub logs: Option<LogsCapabilities>,
    pub upgrade: UpgradeCapabilities,
    pub limits: CapabilityLimits,
}

impl CapabilityReport {
    pub fn new(sections: CapabilityReportSections) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_V1.to_string(),
            agent_id: sections.agent_id,
            instance_id: sections.instance_id,
            reported_at: sections.reported_at,
            exec: sections.exec,
            metrics: sections.metrics,
            logs: sections.logs,
            upgrade: sections.upgrade,
            limits: sections.limits,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
#[serde(deny_unknown_fields)]
pub struct ExecCapabilities {
    #[serde(default)]
    pub opcodes: Vec<String>,
    #[serde(default)]
    pub execution_profiles: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
#[serde(deny_unknown_fields)]
pub struct MetricsCapabilities {
    #[serde(default)]
    pub collectors: Vec<String>,
    #[serde(default)]
    pub scrapers: Vec<String>,
    #[serde(default)]
    pub receivers: Vec<String>,
    #[serde(default)]
    pub discovery_modes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
#[serde(deny_unknown_fields)]
pub struct LogsCapabilities {
    /// 能承接的文件输入形态：`absolute-path` / `glob`（见 `work::FILE_INPUT_*`）。
    ///
    /// 为什么不是布尔：两种形态是**先后落地**的能力（显式路径早就有，单层 glob 是 W1）。
    /// 一个旧 agent 能采 `/var/log/auth.log`，但拿到 `/var/log/postgresql/*` 只能报 unsupported。
    #[serde(default)]
    pub file_inputs: Vec<String>,
    /// 能执行的数据面解析器（首版为空：解析不在 agent 侧）。
    #[serde(default)]
    pub parsers: Vec<String>,
    /// 支持的 `multiline` 模式（`none` / `indented` / …）。
    #[serde(default)]
    pub multiline_modes: Vec<String>,
    /// 支持的监视方式（首版为空）。
    #[serde(default)]
    pub watcher_modes: Vec<String>,
    /// 能跑的定时导出器 ID（`work::EXPORTER_IDS` 的子集）。
    ///
    /// 为什么单独一个桶而不是并进 `file_inputs`：导出器是**具名命令**，不是路径形态 ——
    /// 新加一个导出器（如 W3 的 `journalctl-kernel`）不需要新形态，但需要 agent 真的认识它。
    #[serde(default)]
    pub exporters: Vec<String>,
    /// 能承接的容器选源过滤器**维度**（`work::CONTAINER_FILTERS` 的子集：
    /// `namespace_in` / `pod_not_in`）。
    ///
    /// 为什么必须单独一轴：`WorkSpecSource` 是 `deny_unknown_fields` 的，带 `filters` 的 spec
    /// 遇到不认识的 agentd 会**整份拒收**（不是少采一条，是全停）。网关据此在派活时裁掉
    /// 那条来源，而不是把整份工作发下去换一个 `unsupported`。
    ///
    /// 按**维度**而不是一个布尔：过滤器以后还会长（label 选择是独立字段，见 G11），
    /// 老 agent 声明了 `namespace_in` 不应该因此被当成「也认识 label」。
    #[serde(default)]
    pub filters: Vec<String>,
    /// 能承接的噪音行丢弃方式（`work::DROP_IF_CONTAINS_MODES` 的子集，目前只有 `substring`）。
    ///
    /// 与 `filters` 同一条纪律：`WorkSpecSource.drop_if_contains` 也是新字段，
    /// 旧 agentd 收到就 `deny_unknown_fields` 整份拒收 —— 网关必须在派活前裁掉来源。
    #[serde(default)]
    pub drop_if_contains: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
#[serde(deny_unknown_fields)]
pub struct UpgradeCapabilities {
    pub supported: bool,
    #[serde(default)]
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Contract")]
#[serde(deny_unknown_fields)]
pub struct CapabilityLimits {
    pub max_running_actions: Option<u32>,
    pub max_stdout_bytes: Option<u64>,
    pub max_stderr_bytes: Option<u64>,
    pub max_memory_bytes: Option<u64>,
    pub max_metrics_targets: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sections() -> CapabilityReportSections {
        CapabilityReportSections {
            agent_id: "agent-1".to_string(),
            instance_id: "inst-1".to_string(),
            reported_at: "2026-09-27T00:00:00Z".to_string(),
            exec: ExecCapabilities {
                opcodes: vec!["shell".to_string()],
                execution_profiles: vec!["default".to_string()],
            },
            metrics: MetricsCapabilities {
                collectors: vec!["host".to_string()],
                ..MetricsCapabilities::default()
            },
            logs: None,
            upgrade: UpgradeCapabilities {
                supported: true,
                features: vec!["atomic".to_string()],
            },
            limits: CapabilityLimits {
                max_running_actions: Some(1),
                ..CapabilityLimits::default()
            },
        }
    }

    #[test]
    fn logs_capabilities_round_trip_and_optional_buckets_stay_empty() {
        // 旧 agent 不报 `exporters` / `filters`：`#[serde(default)]` 解成空集，不报错。
        let legacy = r#"{"file_inputs":["absolute-path"]}"#;
        let logs: LogsCapabilities = serde_json::from_str(legacy).expect("decode");
        assert_eq!(logs.file_inputs, vec!["absolute-path".to_string()]);
        assert!(logs.exporters.is_empty());
        assert!(logs.filters.is_empty());

        let full = LogsCapabilities {
            file_inputs: vec!["glob".to_string()],
            exporters: vec!["journalctl-kernel".to_string()],
            filters: vec!["namespace_in".to_string(), "pod_not_in".to_string()],
            ..LogsCapabilities::default()
        };
        let json = serde_json::to_string(&full).expect("encode");
        assert!(json.contains("journalctl-kernel"));
        assert!(json.contains("namespace_in"));
        let back: LogsCapabilities = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, full);

        // 字段漂移仍要显形（与同族结构一致）。
        let drifted = r#"{"file_inputs":[],"nope":1}"#;
        assert!(serde_json::from_str::<LogsCapabilities>(drifted).is_err());
    }

    #[test]
    fn new_stamps_the_schema_version_and_carries_sections() {
        let report = CapabilityReport::new(sections());
        assert_eq!(report.schema_version, SCHEMA_VERSION_V1);
        assert_eq!(report.agent_id, "agent-1");
        assert!(report.logs.is_none());
    }

    #[test]
    fn report_round_trips_and_rejects_unknown_fields() {
        let report = CapabilityReport::new(sections());
        let json = serde_json::to_string(&report).expect("encode");
        let back: CapabilityReport = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, report);

        let mutated = json.replacen('{', "{\"extra\":1,", 1);
        assert!(serde_json::from_str::<CapabilityReport>(&mutated).is_err());
    }

    #[test]
    fn an_all_none_limits_struct_decodes_from_an_empty_object() {
        // Option 字段在 serde 里天然可缺省（不需要 #[serde(default)]）：
        // 整份 limits 不写也应能解析成全 None，而不是报缺字段。
        let limits: CapabilityLimits = serde_json::from_str("{}").expect("decode");
        assert_eq!(limits, CapabilityLimits::default());
    }
}
