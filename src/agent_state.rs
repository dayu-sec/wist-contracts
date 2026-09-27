//! Agent runtime state contract types.

use serde::{Deserialize, Serialize};

use crate::SCHEMA_VERSION_V1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentRuntimeState {
    pub schema_version: String,
    pub agent_id: String,
    pub instance_id: String,
    pub version: String,
    #[serde(default)]
    pub credential_id: Option<String>,
    #[serde(default)]
    pub bearer_token: Option<String>,
    #[serde(default)]
    pub credential_expires_at: Option<String>,
    pub mode: RuntimeMode,
    pub updated_at: String,
}

impl AgentRuntimeState {
    pub fn new(
        agent_id: String,
        instance_id: String,
        version: String,
        mode: RuntimeMode,
        updated_at: String,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_V1.to_string(),
            agent_id,
            instance_id,
            version,
            credential_id: None,
            bearer_token: None,
            credential_expires_at: None,
            mode,
            updated_at,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeMode {
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "degraded")]
    Degraded,
    #[serde(rename = "protect")]
    Protect,
    #[serde(rename = "upgrade_in_progress")]
    UpgradeInProgress,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> AgentRuntimeState {
        AgentRuntimeState::new(
            "agent-1".to_string(),
            "inst-1".to_string(),
            "0.1.5".to_string(),
            RuntimeMode::Normal,
            "2026-09-27T00:00:00Z".to_string(),
        )
    }

    #[test]
    fn new_stamps_schema_version_and_leaves_credentials_empty() {
        let state = state();
        assert_eq!(state.schema_version, SCHEMA_VERSION_V1);
        assert_eq!(state.credential_id, None);
        assert_eq!(state.bearer_token, None);
        assert_eq!(state.credential_expires_at, None);
    }

    #[test]
    fn runtime_mode_uses_the_wire_names_and_rejects_unknown_variants() {
        for (mode, name) in [
            (RuntimeMode::Normal, "normal"),
            (RuntimeMode::Degraded, "degraded"),
            (RuntimeMode::Protect, "protect"),
            (RuntimeMode::UpgradeInProgress, "upgrade_in_progress"),
        ] {
            assert_eq!(serde_json::to_string(&mode).unwrap(), format!("\"{name}\""));
            assert_eq!(
                serde_json::from_str::<RuntimeMode>(&format!("\"{name}\"")).unwrap(),
                mode
            );
        }
        assert!(serde_json::from_str::<RuntimeMode>("\"sleeping\"").is_err());
    }

    #[test]
    fn state_round_trips_with_credentials_and_rejects_unknown_fields() {
        let mut state = state();
        state.credential_id = Some("cred-1".to_string());
        state.bearer_token = Some("wic_x".to_string());
        state.credential_expires_at = Some("2026-10-01T00:00:00Z".to_string());

        let json = serde_json::to_string(&state).expect("encode");
        let back: AgentRuntimeState = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, state);

        let mutated = json.replacen('{', "{\"extra\":1,", 1);
        assert!(serde_json::from_str::<AgentRuntimeState>(&mutated).is_err());
    }

    #[test]
    fn a_legacy_state_without_credentials_still_decodes() {
        // 旧版本可能不写这三个字段（虽然它们带了 #[serde(default)]）。
        let json = r#"{"schema_version":"v1","agent_id":"a","instance_id":"i",
                       "version":"0.1.0","mode":"normal","updated_at":"t"}"#;
        let state: AgentRuntimeState = serde_json::from_str(json).expect("decode");
        assert_eq!(state.credential_id, None);
    }
}
