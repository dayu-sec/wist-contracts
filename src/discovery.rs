//! Discovery runtime contract types shared by edge modules.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::SCHEMA_VERSION_V1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Observed", module = "Observed.Snapshot")]
#[serde(deny_unknown_fields)]
pub struct DiscoverySnapshot {
    pub schema_version: String,
    pub snapshot_id: String,
    pub revision: i64,
    pub generated_at: String,
    #[serde(default)]
    pub origins: Vec<DiscoveryOrigin>,
    #[serde(default)]
    pub resources: Vec<DiscoveredResource>,
    #[serde(default)]
    pub targets: Vec<DiscoveredTarget>,
}

impl DiscoverySnapshot {
    pub fn new(snapshot_id: String, revision: i64, generated_at: String) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_V1.to_string(),
            snapshot_id,
            revision,
            generated_at,
            origins: Vec::new(),
            resources: Vec::new(),
            targets: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Observed", module = "Observed.Snapshot")]
#[serde(deny_unknown_fields)]
pub struct DiscoveryOrigin {
    pub origin_id: String,
    pub probe: String,
    pub source: String,
    pub observed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Observed", module = "Observed.Snapshot")]
#[serde(deny_unknown_fields)]
pub struct DiscoveryCacheMeta {
    pub schema_version: String,
    pub snapshot_id: String,
    pub revision: i64,
    pub generated_at: String,
    #[serde(default)]
    pub origins: Vec<DiscoveryOrigin>,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
}

impl DiscoveryCacheMeta {
    pub fn new(
        snapshot_id: String,
        revision: i64,
        generated_at: String,
        origins: Vec<DiscoveryOrigin>,
        last_success_at: Option<String>,
        last_error: Option<String>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_V1.to_string(),
            snapshot_id,
            revision,
            generated_at,
            origins,
            last_success_at,
            last_error,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Observed", module = "Observed.Snapshot")]
#[serde(deny_unknown_fields)]
pub struct DiscoveredResource {
    pub resource_id: String,
    pub kind: String,
    pub origin_idx: usize,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
    /// When this resource was first discovered (RFC3339 UTC).
    #[serde(default)]
    pub discovered_at: String,
    /// When this resource was last confirmed present (RFC3339 UTC).
    #[serde(default)]
    pub last_seen_at: String,
    /// Health status from the discovery runtime's perspective.
    #[serde(default = "default_health")]
    pub health: String,
    /// Name of the probe that discovered this resource, e.g. "local_runtime".
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Observed", module = "Observed.Snapshot")]
#[serde(deny_unknown_fields)]
pub struct DiscoveredTarget {
    pub target_id: String,
    pub kind: String,
    pub origin_idx: usize,
    pub resource_ref: String,
    #[serde(default)]
    pub execution_hints: BTreeMap<String, String>,
    /// Target state from the discovery runtime's perspective.
    #[serde(default = "default_target_state")]
    pub state: String,
}

fn default_health() -> String {
    "unknown".to_string()
}

fn default_target_state() -> String {
    "active".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Observed", module = "Observed.Snapshot")]
#[serde(deny_unknown_fields)]
pub struct CollectionCandidate {
    pub candidate_id: String,
    pub target_ref: String,
    pub collection_kind: String,
    pub resource_ref: String,
    #[serde(default)]
    pub execution_hints: Vec<StringKeyValue>,
    pub generated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Observed", module = "Observed.Snapshot")]
#[serde(deny_unknown_fields)]
pub struct StringKeyValue {
    pub key: String,
    pub value: String,
}

impl StringKeyValue {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_resource_without_the_optional_fields_decodes_with_stable_defaults() {
        let json = r#"{"resource_id":"r","kind":"host","origin_idx":0}"#;
        let resource: DiscoveredResource = serde_json::from_str(json).expect("decode");
        assert_eq!(resource.health, "unknown");
        assert_eq!(resource.discovered_at, "");
        assert_eq!(resource.last_seen_at, "");
        assert_eq!(resource.source, "");
        assert!(resource.attributes.is_empty());
    }

    #[test]
    fn a_target_defaults_to_active_and_keeps_its_hints() {
        let json = r#"{"target_id":"t","kind":"endpoint","origin_idx":1,
                       "resource_ref":"r","execution_hints":{"addr":"1.2.3.4"}}"#;
        let target: DiscoveredTarget = serde_json::from_str(json).expect("decode");
        assert_eq!(target.state, "active");
        assert_eq!(
            target.execution_hints.get("addr").map(String::as_str),
            Some("1.2.3.4")
        );
    }

    #[test]
    fn resource_and_target_reject_unknown_fields() {
        assert!(
            serde_json::from_str::<DiscoveredResource>(
                r#"{"resource_id":"r","kind":"host","origin_idx":0,"nope":1}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<DiscoveredTarget>(
                r#"{"target_id":"t","kind":"endpoint","origin_idx":1,"resource_ref":"r","nope":1}"#
            )
            .is_err()
        );
    }

    #[test]
    fn snapshot_round_trips_with_nested_items() {
        let mut snapshot =
            DiscoverySnapshot::new("snap-1".to_string(), 3, "2026-09-27T00:00:00Z".to_string());
        snapshot.origins.push(DiscoveryOrigin {
            origin_id: "origin-1".to_string(),
            probe: "host".to_string(),
            source: "local_runtime".to_string(),
            observed_at: "2026-09-27T00:00:00Z".to_string(),
        });
        snapshot.resources.push(DiscoveredResource {
            resource_id: "host:1".to_string(),
            kind: "host".to_string(),
            origin_idx: 0,
            attributes: BTreeMap::from([("host.id".to_string(), "host-01".to_string())]),
            discovered_at: "2026-09-27T00:00:00Z".to_string(),
            last_seen_at: "2026-09-27T00:00:05Z".to_string(),
            health: "healthy".to_string(),
            source: "local_runtime".to_string(),
        });
        snapshot.targets.push(DiscoveredTarget {
            target_id: "host:1:listen".to_string(),
            kind: "endpoint".to_string(),
            origin_idx: 0,
            resource_ref: "host:1".to_string(),
            execution_hints: BTreeMap::new(),
            state: "active".to_string(),
        });

        let json = serde_json::to_string(&snapshot).expect("encode");
        let back: DiscoverySnapshot = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, snapshot);
    }

    #[test]
    fn string_key_value_new_accepts_owned_or_borrowed() {
        let kv = StringKeyValue::new("k", String::from("v"));
        assert_eq!(kv.key, "k");
        assert_eq!(kv.value, "v");
    }
}
