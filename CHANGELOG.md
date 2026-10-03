# 更新日志

本文件记录 `wist-contracts` 的所有重要变更。格式遵循 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)，
版本号遵循[语义化版本](https://semver.org/lang/zh-CN/)。

## [0.1.14] - 2026-10-03

### 新增

- **Agent 状态上报新增可选字段 `machine_profile`**（机器名 / `node_id` / `machine_id` / 网卡地址）：
  管理面据此在注册表里显示「这是哪台机器」。旧 agent 不带该字段仍可解码。

## [0.1.13] - 2026-10-03

### 新增

- **`Exporter`（定时导出器）来源进入「可执行」判定**：采集内容目录里的 `Exporter` 来源不再一律
  被当成「接不了」。新增词表 `EXPORTER_IDS`（已实现的导出器：`journalctl-unit` /
  `journalctl-shutdown` / `last-reboot` / `nft-ruleset` / `iptables-save` / `smartctl` / `dmesg` /
  `auditd-execve`），以及 `parse_exporter_target`（`id:arg` 归一）与 `is_known_exporter`。
- **`is_executable_source` 认 `Exporter`**：目标是**已知导出器 ID**（可带 `:arg`）即可采；
  未知 ID 一律不可采。`EXECUTABLE_SOURCE_KINDS` 随之加入 `Exporter`。

### 说明

- 这次只改「**能不能采**」这一轴：采集就绪度（单元 `status`）与解析就绪度（单元 `rule_ref`）
  的口径不变。导出器只解决「采得到」。
- 这个判据被**网关与 agentd 共用**（`is_executable_source`）：消费方要一起升级，
  否则会出现「网关说可采、agent 拿到后报 unsupported」的不一致。

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
