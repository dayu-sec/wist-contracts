//! Agent enrollment / identity **domain** types.
//!
//! 跨进程 seam 的**报文**（`EnrollmentRequest` / `EnrollmentEnvelope` / `EnrollmentOutcome` /
//! `EnrollmentStatus` / `CredentialRenewal` / `CredentialRenewed`）已迁到独立 seam crate
//! `wist-api::enrollment`。这里保留的是被多条 seam 复用的**领域类型**（如 `HostProfile` 同时
//! 用在 `agent/enroll` 与 `agent/status`），故留在两侧共同底座。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Control", module = "Control.Agent.Identity")]
#[serde(deny_unknown_fields)]
pub struct HostProfile {
    pub node_id: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub machine_id: String,
    pub cloud_instance_id: Option<String>,
    pub k8s_node_uid: Option<String>,
    pub ip_addresses: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Control", module = "Control.Agent.Identity")]
#[serde(deny_unknown_fields)]
pub struct AgentIdentity {
    pub agent_id: String,
    pub instance_id: String,
    pub tenant_id: String,
    pub environment_id: String,
    pub node_id: String,
    pub issued_at: String,
    pub expires_at: Option<String>,
    pub status: AgentIdentityStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "state", domain = "Control", module = "Control.Agent.Identity")]
pub enum AgentIdentityStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "revoked")]
    Revoked,
    #[serde(rename = "expired")]
    Expired,
    #[serde(rename = "renewal_required")]
    RenewalRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Control", module = "Control.Agent.Identity")]
#[serde(deny_unknown_fields)]
pub struct CredentialBundle {
    pub credential_id: String,
    pub agent_id: String,
    pub instance_id: String,
    /// 客户端证书（PEM）。mTLS 是 agent 与网关之间的**唯一**凭据路径（bearer 已删），
    /// 所以这里是必填：注册/续期都必须换回一张证书。
    pub certificate: String,
    pub private_key_ref: Option<String>,
    pub ca_bundle: Option<String>,
    pub issued_at: String,
    pub not_before: Option<String>,
    pub not_after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitialConfig {
    pub schema_version: String,
    pub mode: String,
    pub gateway_endpoint: String,
    pub policy_version: String,
    pub telemetry_output: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyBinding {
    pub agent_id: String,
    pub policy_id: String,
    pub policy_version: String,
    pub bound_at: String,
}

#[cfg(test)]
mod tests {
    use super::{CredentialBundle, HostProfile};

    /// 契约收口（2026-09-30）：agent 的凭据包**只剩客户端证书** —— 线上不再有 `bearer_token` /
    /// `auth_scheme`，而 `certificate` 是必填。带旧字段的老报文一律拒（`deny_unknown_fields`）。
    #[test]
    fn credential_bundle_carries_only_the_client_certificate() {
        let bundle = CredentialBundle {
            credential_id: "cred-1".to_string(),
            agent_id: "agent-1".to_string(),
            instance_id: "inst-1".to_string(),
            certificate: "CERT".to_string(),
            private_key_ref: None,
            ca_bundle: None,
            issued_at: "2026-09-30T00:00:00Z".to_string(),
            not_before: None,
            not_after: None,
        };
        let json = serde_json::to_string(&bundle).expect("encode");
        assert!(!json.contains("bearer_token"), "{json}");
        assert!(!json.contains("auth_scheme"), "{json}");

        // 带旧字段的老报文解不了（字段已从契约里删掉）。
        let legacy = r#"{"credential_id":"c","agent_id":"a","instance_id":"i",\
            "auth_scheme":"bearer","bearer_token":"wic_x","certificate":"CERT",\
            "private_key_ref":null,"ca_bundle":null,"issued_at":"t",\
            "not_before":null,"not_after":null}"#;
        assert!(serde_json::from_str::<CredentialBundle>(legacy).is_err());

        // certificate 必填：缺了就解不了（mTLS 是唯一凭据路径，没有它就没有凭据）。
        let no_certificate = r#"{"credential_id":"c","agent_id":"a","instance_id":"i",\
            "private_key_ref":null,"ca_bundle":null,"issued_at":"t",\
            "not_before":null,"not_after":null}"#;
        assert!(serde_json::from_str::<CredentialBundle>(no_certificate).is_err());
    }

    #[test]
    fn host_profile_round_trips_and_rejects_unknown_fields() {
        let profile = HostProfile {
            node_id: "node-1".to_string(),
            hostname: "host-1".to_string(),
            os: "macos".to_string(),
            arch: "arm64".to_string(),
            machine_id: "mid-1".to_string(),
            cloud_instance_id: None,
            k8s_node_uid: None,
            ip_addresses: vec!["10.0.0.1".to_string()],
        };
        let json = serde_json::to_string(&profile).expect("encode");
        let back: HostProfile = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, profile);

        let mutated = json.replacen('{', "{\"extra\":1,", 1);
        assert!(serde_json::from_str::<HostProfile>(&mutated).is_err());
    }
}
