//! Gateway ↔ center control contract types (gateway registration & credential, mTLS).
//!
//! 与 agent 侧 `enrollment.rs` **对称**：网关对中心的注册/凭据**唯一路径是客户端证书（mTLS）**，
//! 运行期 bearer 已删（`auth_scheme` / `bearer_token` 不再出现在线上）。
//!
//! 本模块**手工维护**：对应的模型 `Control.Gateway.{Security,Supervision}` 生成物（`wist-control`）
//! 受 `jumo-code generate` 阻塞（与本改动无关；见 `wist-gateway/docs/design/agent-identity-mtls.md` §7）。
//! 解除阻塞后可用 `jumo-code align` 回填注解、届时再考虑把生成物并入。

use serde::{Deserialize, Serialize};

/// 网关注册请求：持一次性引导 Token + 网关本地生成的 CSR。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(
    kind = "message",
    role = "command",
    domain = "Control",
    module = "Control.Gateway.Supervision"
)]
#[serde(deny_unknown_fields)]
pub struct RegisterGateway {
    pub enrollment_token: String,
    pub instance_id: String,
    /// 网关本地生成的 **CSR**（PEM）。mTLS 是唯一凭据路径，注册**必须**带 CSR——
    /// 中心据此签一张客户端证书；没有它就没有凭据可用（不再回落 bearer）。
    /// 私钥**永不上送**，只交公钥；主体由中心填（`gateway_id`）。
    pub certificate_signing_request: String,
    pub requested_at: String,
}

/// 网关客户端证书（长期身份）：由网关专属 CA（CA-G）签发、每网关一张、绑定 `gateway_id`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(
    kind = "struct",
    domain = "Control",
    module = "Control.Gateway.Security"
)]
#[serde(deny_unknown_fields)]
pub struct GatewayClientCertificate {
    pub certificate_id: String,
    pub gateway_id: String,
    pub serial: String,
    /// 客户端证书（PEM）。
    pub certificate: String,
    /// CA-G 信任根（PEM，可选）。
    pub ca_bundle: Option<String>,
    pub issued_at: String,
    pub not_before: Option<String>,
    pub not_after: Option<String>,
    pub status: GatewayClientCertificateStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(
    kind = "state",
    domain = "Control",
    module = "Control.Gateway.Security"
)]
pub enum GatewayClientCertificateStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "expired")]
    Expired,
    #[serde(rename = "revoked")]
    Revoked,
}

/// 注册成功后签发的长期凭据：**只剩客户端证书**（bearer / `auth_scheme` 已删）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(
    kind = "struct",
    domain = "Control",
    module = "Control.Gateway.Security"
)]
#[serde(deny_unknown_fields)]
pub struct GatewayCredentialBundle {
    pub credential_id: String,
    pub gateway_id: String,
    pub instance_id: Option<String>,
    /// 客户端证书（PEM）。mTLS 是网关与中心之间的**唯一**凭据路径，所以这里必填。
    pub certificate: String,
    pub ca_bundle: Option<String>,
    pub issued_at: String,
    pub not_before: Option<String>,
    pub not_after: Option<String>,
}

/// 网关凭据校验结果：以客户端证书校验通过后的记录（`certificate_serial` 为被验证书的序列号）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(
    kind = "struct",
    domain = "Control",
    module = "Control.Gateway.Security"
)]
#[serde(deny_unknown_fields)]
pub struct GatewayCredentialVerificationResult {
    pub gateway_id: String,
    pub certificate_serial: String,
    pub status: String,
    pub verified_at: String,
}

/// 网关注册回执。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayEnrollmentResult {
    pub status: String,
    pub gateway_id: String,
    pub instance_id: String,
    pub credential_id: String,
    pub initial_config: String,
    pub credential_bundle: GatewayCredentialBundle,
}

/// 凭据轮换（到期前）：持当前证书序列号 + 新 CSR，换一张新客户端证书。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(
    kind = "message",
    role = "command",
    domain = "Control",
    module = "Control.Gateway.Supervision"
)]
#[serde(deny_unknown_fields)]
pub struct RenewGatewayCredential {
    pub gateway_id: String,
    pub current_certificate_serial: String,
    /// 轮换时提交的 **CSR**（PEM）。与注册同口径：私钥不上送、主体由中心填。
    pub certificate_signing_request: String,
    pub requested_at: String,
}

/// 校验网关通讯凭据（客户端证书）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(
    kind = "message",
    role = "command",
    domain = "Control",
    module = "Control.Gateway.Supervision"
)]
#[serde(deny_unknown_fields)]
pub struct VerifyGatewayCredential {
    pub gateway_id: String,
    pub certificate_serial: String,
}

#[cfg(test)]
mod tests {
    use super::{
        GatewayClientCertificate, GatewayClientCertificateStatus, GatewayCredentialBundle,
        GatewayCredentialVerificationResult, RegisterGateway, RenewGatewayCredential,
        VerifyGatewayCredential,
    };

    fn bundle() -> GatewayCredentialBundle {
        GatewayCredentialBundle {
            credential_id: "cred-1".to_string(),
            gateway_id: "gw-1".to_string(),
            instance_id: None,
            certificate: "CERT".to_string(),
            ca_bundle: None,
            issued_at: "2026-10-04T00:00:00Z".to_string(),
            not_before: None,
            not_after: Some("2026-11-04T00:00:00Z".to_string()),
        }
    }

    /// 收口：网关凭据包**只剩客户端证书** —— 线上不再有 `bearer_token` / `auth_scheme`，
    /// 而 `certificate` 必填。带旧字段的老报文一律拒（`deny_unknown_fields`）。
    #[test]
    fn gateway_credential_bundle_carries_only_the_client_certificate() {
        let json = serde_json::to_string(&bundle()).expect("encode");
        assert!(!json.contains("bearer_token"), "{json}");
        assert!(!json.contains("auth_scheme"), "{json}");

        let legacy = r#"{"credential_id":"c","gateway_id":"g","instance_id":null,"auth_scheme":"bearer","bearer_token":"rt_x","certificate":"CERT","ca_bundle":null,"issued_at":"t","not_before":null,"not_after":null}"#;
        assert!(serde_json::from_str::<GatewayCredentialBundle>(legacy).is_err());

        let no_certificate = r#"{"credential_id":"c","gateway_id":"g","instance_id":null,"ca_bundle":null,"issued_at":"t","not_before":null,"not_after":null}"#;
        assert!(serde_json::from_str::<GatewayCredentialBundle>(no_certificate).is_err());
    }

    #[test]
    fn register_requires_a_csr_and_rejects_unknown_fields() {
        let request = RegisterGateway {
            enrollment_token: "reg-1".to_string(),
            instance_id: "gw-1/inst-1".to_string(),
            certificate_signing_request: "-----BEGIN CERTIFICATE REQUEST-----\nA\n".to_string(),
            requested_at: "2026-10-04T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&request).expect("encode");
        assert!(json.contains("certificate_signing_request"));
        assert_eq!(
            serde_json::from_str::<RegisterGateway>(&json).expect("decode"),
            request
        );

        // CSR 必填：缺该字段的报文解不了（注册不再有「不带证书」的退路）。
        let mut without_csr = serde_json::to_value(&request).expect("encode");
        without_csr
            .as_object_mut()
            .expect("object")
            .remove("certificate_signing_request");
        assert!(serde_json::from_value::<RegisterGateway>(without_csr).is_err());

        // 未知字段拒收（契约收口）。
        let mutated = json.replacen('{', "{\"extra\":1,", 1);
        assert!(serde_json::from_str::<RegisterGateway>(&mutated).is_err());
    }

    #[test]
    fn renewal_and_verification_use_certificate_fields() {
        let renewal = RenewGatewayCredential {
            gateway_id: "gw-1".to_string(),
            current_certificate_serial: "01".to_string(),
            certificate_signing_request: "CSR".to_string(),
            requested_at: "2026-10-04T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&renewal).expect("encode");
        assert!(json.contains("current_certificate_serial"));
        assert!(json.contains("certificate_signing_request"));

        let verify = VerifyGatewayCredential {
            gateway_id: "gw-1".to_string(),
            certificate_serial: "01".to_string(),
        };
        assert!(
            serde_json::to_string(&verify)
                .expect("encode")
                .contains("certificate_serial")
        );

        let result = GatewayCredentialVerificationResult {
            gateway_id: "gw-1".to_string(),
            certificate_serial: "01".to_string(),
            status: "valid".to_string(),
            verified_at: "2026-10-04T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&result).expect("encode");
        assert!(json.contains("certificate_serial"));
        assert!(!json.contains("credential_id"), "{json}");
    }

    #[test]
    fn client_certificate_status_uses_wire_names() {
        assert_eq!(
            serde_json::to_string(&GatewayClientCertificateStatus::Active).unwrap(),
            "\"active\""
        );
        assert_eq!(
            serde_json::to_string(&GatewayClientCertificateStatus::Revoked).unwrap(),
            "\"revoked\""
        );
        assert!(serde_json::from_str::<GatewayClientCertificateStatus>("\"unknown\"").is_err());

        let cert = GatewayClientCertificate {
            certificate_id: "cert-1".to_string(),
            gateway_id: "gw-1".to_string(),
            serial: "01".to_string(),
            certificate: "PEM".to_string(),
            ca_bundle: None,
            issued_at: "2026-10-04T00:00:00Z".to_string(),
            not_before: None,
            not_after: None,
            status: GatewayClientCertificateStatus::Active,
        };
        let back: GatewayClientCertificate =
            serde_json::from_str(&serde_json::to_string(&cert).unwrap()).unwrap();
        assert_eq!(back, cert);
    }
}
