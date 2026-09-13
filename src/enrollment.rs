//! Agent enrollment contract types.

use serde::{Deserialize, Serialize};

use crate::API_VERSION_V1;

pub const SUBMIT_ENROLLMENT_REQUEST_KIND: &str = "submit_enrollment_request";
pub const RENEW_AGENT_CREDENTIAL_KIND: &str = "renew_agent_credential";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrollmentRequest {
    pub api_version: String,
    pub kind: String,
    pub token: String,
    pub credential_request: String,
    pub host_profile: HostProfile,
    pub capability_summary: String,
    pub requested_at: String,
}

impl EnrollmentRequest {
    pub fn new(
        token: String,
        credential_request: String,
        host_profile: HostProfile,
        capability_summary: String,
        requested_at: String,
    ) -> Self {
        Self {
            api_version: API_VERSION_V1.to_string(),
            kind: SUBMIT_ENROLLMENT_REQUEST_KIND.to_string(),
            token,
            credential_request,
            host_profile,
            capability_summary,
            requested_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrollmentEnvelope {
    pub result: EnrollmentOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnrollmentOutcome {
    pub status: EnrollmentStatus,
    pub reason_code: Option<String>,
    pub agent_id: Option<String>,
    pub instance_id: Option<String>,
    pub issued_identity: Option<AgentIdentity>,
    pub credential_bundle: Option<CredentialBundle>,
    pub initial_config: Option<InitialConfig>,
    pub policy_binding: Option<PolicyBinding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EnrollmentStatus {
    #[serde(rename = "accepted")]
    Accepted,
    #[serde(rename = "rejected")]
    Rejected,
    #[serde(rename = "pending_review")]
    PendingReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialBundle {
    pub credential_id: String,
    pub agent_id: String,
    pub instance_id: String,
    pub auth_scheme: Option<String>,
    pub bearer_token: Option<String>,
    pub certificate: Option<String>,
    pub private_key_ref: Option<String>,
    pub ca_bundle: Option<String>,
    pub issued_at: String,
    pub not_before: Option<String>,
    pub not_after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialRenewal {
    pub api_version: String,
    pub kind: String,
    pub agent_id: String,
    pub instance_id: String,
    pub credential_request: String,
    pub requested_at: String,
}

impl CredentialRenewal {
    pub fn new(
        agent_id: String,
        instance_id: String,
        credential_request: String,
        requested_at: String,
    ) -> Self {
        Self {
            api_version: API_VERSION_V1.to_string(),
            kind: RENEW_AGENT_CREDENTIAL_KIND.to_string(),
            agent_id,
            instance_id,
            credential_request,
            requested_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialRenewed {
    pub credential_bundle: CredentialBundle,
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
    use super::{
        EnrollmentOutcome, EnrollmentEnvelope, EnrollmentStatus,
        RENEW_AGENT_CREDENTIAL_KIND, CredentialRenewal,
    };

    #[test]
    fn enrollment_result_status_uses_wire_names() {
        let decoded: EnrollmentEnvelope =
            serde_json::from_str(r#"{"result":{"status":"accepted","reason_code":null,"agent_id":"agent-1","instance_id":"host-a","issued_identity":null,"credential_bundle":null,"initial_config":null,"policy_binding":null}}"#)
                .expect("decode");

        assert_eq!(decoded.result.status, EnrollmentStatus::Accepted);

        let encoded = serde_json::to_string(&EnrollmentOutcome {
            status: EnrollmentStatus::PendingReview,
            reason_code: Some("manual_review".to_string()),
            agent_id: None,
            instance_id: None,
            issued_identity: None,
            credential_bundle: None,
            initial_config: None,
            policy_binding: None,
        })
        .expect("encode");

        assert!(encoded.contains("\"pending_review\""));
    }

    #[test]
    fn renew_agent_credential_uses_stable_wire_kind() {
        let request = CredentialRenewal::new(
            "agent-a".to_string(),
            "instance-a".to_string(),
            "bearer".to_string(),
            "2026-07-29T00:00:00Z".to_string(),
        );
        let encoded = serde_json::to_string(&request).expect("encode");

        assert!(encoded.contains(&format!("\"kind\":\"{RENEW_AGENT_CREDENTIAL_KIND}\"")));

        let decoded: CredentialRenewal = serde_json::from_str(&encoded).expect("decode");
        assert_eq!(decoded.api_version, "v1");
        assert_eq!(decoded.kind, RENEW_AGENT_CREDENTIAL_KIND);
    }
}
