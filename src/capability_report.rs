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
    #[serde(default)]
    pub file_inputs: Vec<String>,
    #[serde(default)]
    pub parsers: Vec<String>,
    #[serde(default)]
    pub multiline_modes: Vec<String>,
    #[serde(default)]
    pub watcher_modes: Vec<String>,
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
