//! Output envelope for wist data exchange.
//!
//! All external-facing output (discovery snapshots, metrics batches,
//! and future event batches) uses this envelope. The envelope provides
//! source identity, idempotency fields (`output_id` + `seq`), and
//! kind-based payload routing for downstream consumers (wist data plane).

use serde::{Deserialize, Serialize};

/// Unified output envelope wrapping any payload type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Pipeline")]
#[serde(deny_unknown_fields)]
pub struct ExporterOutput<T> {
    /// Schema identifier for the envelope itself.
    /// Target spec value: "wist/v1".
    pub api_version: String,
    /// Payload type discriminator: "disc_snap" / "metrics".
    pub kind: String,
    /// Globally unique output identifier.
    /// Format: `<agent_id>_<seq>` or `<seq>` when agent_id is unavailable.
    pub output_id: String,
    /// Monotonically increasing sequence number within daemon lifetime.
    pub seq: u64,
    /// When this output was generated (RFC3339 UTC).
    pub generated_at: String,
    /// Source agent identity.
    pub source: ExporterSource,
    /// Kind-specific payload.
    pub payload: T,
}

/// Source agent identity within the envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Reporting", module = "Reporting.Pipeline")]
#[serde(deny_unknown_fields)]
pub struct ExporterSource {
    /// Logical agent identity, stable across instances.
    pub agent_id: String,
    /// Current daemon run instance, for upgrade/replacement tracking.
    pub instance_id: String,
    /// Probe kind that produced this output, e.g. "host", "process", "container".
    /// Present for disc_snap outputs; absent for metrics outputs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe: Option<String>,
}

impl ExporterSource {
    pub fn new(agent_id: &str, instance_id: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            instance_id: instance_id.to_string(),
            probe: None,
        }
    }

    pub fn with_probe(mut self, probe: &str) -> Self {
        self.probe = Some(probe.to_string());
        self
    }
}

impl<T> ExporterOutput<T> {
    pub fn new(
        kind: &str,
        output_id: String,
        seq: u64,
        generated_at: String,
        source: ExporterSource,
        payload: T,
    ) -> Self {
        Self {
            api_version: EXPORTER_API_VERSION.to_string(),
            kind: kind.to_string(),
            output_id,
            seq,
            generated_at,
            source,
            payload,
        }
    }
}

pub const EXPORTER_API_VERSION: &str = "wist/v1";

#[cfg(test)]
mod tests {
    use super::*;

    fn output() -> ExporterOutput<serde_json::Value> {
        ExporterOutput::new(
            "disc_snap",
            "agent-1_7".to_string(),
            7,
            "2026-09-27T00:00:00Z".to_string(),
            ExporterSource::new("agent-1", "inst-1"),
            serde_json::json!({"snapshot_id": "s-1"}),
        )
    }

    #[test]
    fn new_stamps_the_shared_api_version_and_kind() {
        let out = output();
        assert_eq!(out.api_version, EXPORTER_API_VERSION);
        assert_eq!(out.kind, "disc_snap");
        assert_eq!(out.seq, 7);
    }

    #[test]
    fn a_source_without_a_probe_omits_the_key_and_still_decodes() {
        // 指标输出没有 probe：这个键不该出现（否则旧消费端会读到空探针名）。
        let out = output();
        let json = serde_json::to_string(&out).expect("encode");
        assert!(!json.contains("probe"), "{json}");
        let back: ExporterOutput<serde_json::Value> = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, out);
    }

    #[test]
    fn a_probe_is_carried_when_set() {
        let out = ExporterOutput::new(
            "disc_snap",
            "agent-1_8".to_string(),
            8,
            "2026-09-27T00:00:01Z".to_string(),
            ExporterSource::new("agent-1", "inst-1").with_probe("host"),
            serde_json::json!({}),
        );
        let json = serde_json::to_string(&out).expect("encode");
        assert!(json.contains("\"probe\":\"host\""), "{json}");
    }

    #[test]
    fn the_envelope_and_the_source_reject_unknown_fields() {
        let json = serde_json::to_string(&output()).expect("encode");
        let envelope_extra = json.replacen('{', "{\"extra\":1,", 1);
        assert!(
            serde_json::from_str::<ExporterOutput<serde_json::Value>>(&envelope_extra).is_err()
        );

        let source_extra = json.replacen(
            "\"instance_id\":\"inst-1\"",
            "\"instance_id\":\"inst-1\",\"extra\":1",
            1,
        );
        assert!(serde_json::from_str::<ExporterOutput<serde_json::Value>>(&source_extra).is_err());
    }
}
