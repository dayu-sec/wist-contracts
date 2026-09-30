# 更新日志

本文件记录 `wist-contracts` 的所有重要变更。格式遵循 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)，
版本号遵循[语义化版本](https://semver.org/lang/zh-CN/)。

## [0.1.12] - 2026-09-30

### 移除

- **`CredentialBundle` 去掉 `bearer_token` / `auth_scheme`**：agent 与网关之间只剩客户端证书
  一种凭据，注册回包不再下发 bearer token。
- **`ControlPlaneSection` / `AgentRuntimeState` 去掉 `bearer_token`**：agent 侧不再保存 bearer 凭据。

### 变更

- **`CredentialBundle.certificate` 由可选改为必填**：注册/续期都必须换回一张客户端证书。
- **`EnrollmentRequest` / `CredentialRenewal` 的 `certificate_signing_request` 由可选改为必填**：
  注册与续期都必须带 CSR（私钥仍不上送，主体由网关填）。

**破坏性**：带旧字段（`bearer_token` / `auth_scheme`）或不带 `certificate` / CSR 的报文将**无法解析**；
消费方需同步升级。
