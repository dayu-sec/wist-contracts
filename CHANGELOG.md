# 更新日志

本文件记录 `wist-contracts` 的所有重要变更。格式遵循 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)，
版本号遵循[语义化版本](https://semver.org/lang/zh-CN/)。

## [0.9.5] - 2026-10-24

### 新增（G1 能力轴 + G3 记录边界）

- **`LogsCapabilities.filters`**（默认空）：agent 能承接的容器过滤**维度**
  （`namespace_in` / `pod_not_in`，取值来自 `CONTAINER_FILTERS`）。
  为什么必须单列一轴：`WorkSpecSource` 是 `deny_unknown_fields` 的，带 `filters` 的 spec 落到
  不认识的 agentd 会被**整份拒收**（不是少采一条，是全停）。
- **`LogsCapabilities.drop_if_contains`**（默认空，目前只有 `substring`）：能承接的噪音行
  丢弃方式。与 `filters` 同一条纪律（新字段 = 新能力）。
- **`WorkSpecSource.drop_if_contains`**（默认空，不序列化）：含这些**精确子串**的记录整条丢弃。
  配套 `validate_drop_if_contains`（条数 / 长度 / 空串 / 控制字符 / 重复），网关、agentd、
  页面共用同一口径。
- **`MULTILINE_MODES` 增 `mysql_slow` / `postgres_csv`**：MySQL slow log（`# Time:` 起块）
  与 PostgreSQL csvlog（**引号外**的换行才切记录）。
- **`missing_log_capabilities` 改为收整条来源**（`&WorkSpecSource`）：每加一个选源 / 读法字段
  都要重新讨论“要不要传”，而“忘了传”的后果是整份 spec 被拒收。收整条来源后，新增字段时
  编译器会把所有调用点指出来。
- 常量 `FILTER_NAMESPACE_IN` / `FILTER_POD_NOT_IN` / `CONTAINER_FILTERS` /
  `DROP_IF_CONTAINS_SUBSTRING` / `DROP_IF_CONTAINS_MODES`；`ContainerFilters::used_dimensions`
  与能力词表同源。

### 变更（事实摘要升 `fact-v3`）

- **`FACT_DIGEST_VERSION` → `fact-v3`**：内容摘要的字段集变了，前缀必须跟着换，否则新旧两侧
  对同一台机器会**静默**给出不同的「没变」。代价是升级后全网重新上报一次事实
  （预期行为，不是故障）。
- **`listen_ports` 只报稳定服务端口**：形如 `tcp:5432`，滤掉内核临时端口区间
  （`>= EPHEMERAL_PORT_START`）。实测一台机器 246 个监听里 220 个是客户端 socket、端口复用池
  与转发的副产物 —— 它们随无关进程生灭换号，等于让幂等键失效。
- 新增常量 `EPHEMERAL_PORT_START`（32768）：agentd 用它决定哪些监听不上报，网关用它把
  「没监听」×「这个区间没采」分开 —— 两侧共用一份，否则同一条事实会得出不同的三态结论。

### 兼容性

- **`fact-v3` 会触发一次全网重报**：摘要前缀变了，缓存里的旧摘要全部失效。
- 只增字段 / 词表项，老数据仍能解析；但**反方向需要发布顺序**：
  `AgentStatusReport` 与 `WorkSpecSource` 都是 `deny_unknown_fields` 的 —— 老网关收到新
  `logs_capabilities` 会拒收整份状态上报，老 agentd 收到新 `spec` 字段会拒收整份工作。
  现场滚动顺序是：**contracts → 网关（先能收）→ agentd → web → 目录包**；
  编译上的依赖顺序仍是 contracts 先于其余所有仓。网关对 `drop_if_contains` 做了**降级**
  （agent 不支持/未上报时去掉字段继续采），所以新的减法字段不再会导致整份工作拒收。

## [0.9.4] - 2026-10-23

### 新增（D7 能力协商）

- **`LogsCapabilities` 增 `exporters`**（默认空）：agent 能跑的导出器 ID，取值是
  `EXPORTER_IDS` 的子集。为什么单独一个桶而不并进 `file_inputs`：导出器是**具名命令**，
  不是路径形态 —— 新增一个导出器（如 W3 的 `journalctl-kernel`）不需要新形态，
  但需要 agent 真的认识它。
- **`missing_log_capabilities(kind, target, multiline, declared)`**：一条来源还缺哪些能力
  （`file_inputs:*` / `multiline_modes:*` / `exporters:*`；空 = 齐了）。
  `None` = 这个 kind 不归 logs 能力管（指标；不可执行的形态由 `is_executable_source` 说）。
  网关（派活时）与 agentd（上报时）用**同一份**判定，避免两侧各写一份名单而漂。
- 常量 `FILE_INPUT_ABSOLUTE_PATH` / `FILE_INPUT_GLOB` / `MULTILINE_MODES`：与
  `is_file_target` / `WorkSpecSource::multiline` 同源。

### 兼容性

- 只增字段（`#[serde(default)]`）：老网关发的 / 老 agent 收的报告少 `exporters` 仍能解析。
- ⚠ 反方向不兼容：`AgentStatusReport` 是 `deny_unknown_fields` 的，所以**网关必须先升级**
  （先能收），agentd 才能开始上报能力；这是 D7 的两阶段发布纪律。

## [0.9.3] - 2026-10-23

### 新增

- **`EXPORTER_IDS` 增 `journalctl-kernel`**（`KERNEL_LOG_EXPORTER` 常量）：内核日志的
  「定时导出器」来源，`journalctl -k -p 4 -o json`，**靠游标续读**。
  它是词表里第一个**有状态**的导出器 —— 契约只承诺「这个 ID 可执行」，
  游标落在哪、失效（被 journal vacuum）怎么回退由采集端负责。
  为什么需要一个具名常量：agentd 的 argv 与状态处理要按这个 ID 分支，
  字面量散在两侧迟早会漂（改名后一处还在按旧名特判，表现就是「游标不生效、每轮重读」）。

### 兼容性

- 只增词表项，未改任何字段：消费方仍用 `version = "0.9"` 解析到 0.9.3。
- ⚠ **旧 agentd 会把这个 ID 当未知导出器**并如实报 `unsupported`（不当没看见）：
  所以知识库把内核单元改成这个来源之前，**必须先把 agentd 升上去**；
  否则该面在旧机队上会从「能采」变成「报不支持」。
- ⚠ **旧网关更严**：它不认识这个 ID 时，`active` 单元会被装载期校验拦下（整份目录加载失败），
  所以发布顺序是 contracts → agentd → 网关 → 知识库。

## [0.9.2] - 2026-10-23

### 新增

- **`WorkSpecSource` 增 `filters: ContainerFilters`**（默认空，只对 `FileGlob` 有意义）：
  容器选源的**结构化过滤器**（W2），`namespace_in`（白名单）/ `pod_not_in`（黑名单）。
  与 `exclude_globs` 的分工：排除项是按**展开后的完整路径**匹配的 glob，会随集群里
  pod 名字的形态漂移；过滤器只依赖 `\<pod>_\<ns>_\<container>-\<id>.log` 这个文件名约定，
  由采集端在**发现阶段**求值（读取器不参与）。
  两个列表都空时**整字段不序列化**（与 `exclude_globs` 同一条纪律：旧 agent 的
  `WorkSpecSource` 是 `deny_unknown_fields` 的）。
- `ContainerFilters::is_empty()` / `ContainerFilters::validate()`：后者把“一定匹配不上”的形状
  （空串 / 首尾空白 / 通配符 / >253）拒在最早的入口，网关、web、agentd 三处共用一份。
- `CONTAINER_LOG_ROOT = "/var/log/containers"`：选源根写死（D4）。`/var/log/pods` 下
  有 `0.log` / `1.log` 与重启序号歧义，而这里每个活容器恰好一条 symlink、文件名带
  pod / ns / container。

### 兼容性

- 只增字段：老网关发来的 spec 没有 `filters`，`#[serde(default)]` 解成空过滤器；
  消费方仍可用 `version = "0.9"` 解析到 0.9.2。
- ⚠ **反方向不兼容**：`filters` 非空时要求 agent 支持（D7 能力上报未落地前，
  目录里带 filters 的容器单元保持 `draft`，机队升级验证后再单独翻 `active`）。

## [0.9.1] - 2026-10-22

### 新增

- **文件族 glob 成为可执行的 `FileGlob` 目标**：判据从「显式绝对路径」放宽为
  「绝对路径，允许通配、不允许 `**`」。新增 `is_glob_path()` / `is_file_target()`；
  `is_executable_source("FileGlob", …)` 改用后者，`is_explicit_path()` 保留给单文件语义
  （存在性裁剪只能对显式路径做）。`**` 被拒是有意的：采集端只扫 pattern 的固定前缀目录、
  `*` 不跨目录，递归展开既没实现也给不出上限。
- **`WorkSpecSource` 增 `exclude_globs`**（默认空，只对 `FileGlob` 有意义，按展开后的完整路径匹配）：
  容器日志的 v1 选源靠它 —— `/var/log/containers/*.log` 再排除不该采的容器。
  空数组**不序列化**：旧 agent 的 `WorkSpecSource` 是 `deny_unknown_fields` 的，
  发一个空字段过去只会给旧机队添一种没必要的解析失败。

> 兼容性：只增字段 + 放宽判据，不改既有线上 JSON；消费方仍可用 `version = "0.9"` 解析到 0.9.1。
> 但**新能力要秩序**：glob 单元只有在机队上报了对应能力后才应翻 `active`（D7）。

## [0.9.0] - 2026-10-18

### 新增

- **事实摘要支持路径存在性**：`FactContent` 增加 `probed_paths` / `present_paths`，
  规范化与摘要版本升到 **`fact-v2`**（字段集变了，升级后强制重报一次）。
  两列必须成对：只有「探测过」与「存在」分开，`path_exists` 的三态求值才成立
  （没探测 = 无法判定，探测过但不存在 = 不命中）。
- **发现策略表增加 `probe_paths`**：网关从采集目录的 `path_exists:` 条件推导后下发，
  agentd 按白名单逐条探存在性。空 = 不探测（旧网关 / 目录无路径条件）。

## [0.8.0] - 2026-10-11

### 变更（不兼容）

- **`agent_config` 模块移出到 `wist-agentd`**：agent 的**本地运行配置**（`agentd.toml` 的 `AgentConfig`
  及各段）不属于「edge ↔ center 交换的契约」，已迁入**唯一生产消费者** `wist-agentd`
  （`wist_agentd::config::agent_config`）。`wist-contracts` 不再提供 `agent_config` 模块。
- **配置校验 `validate_config` 随之归位**到 `wist-agentd::config::validate`；`wist-validate` 不再提供
  `config` 模块（其 config fixture 测试也一并迁入 `wist-agentd`）。
- **线上 JSON 不变**：本次只搬位置、未改字段，但**引用该模块的消费方需一起升级**。
  `wist-agentd` / `wist-gateway` 已同步。

## [0.7.0] - 2026-10-06

### 变更（不兼容）

- **`gateway_control` 模块删除**：网关面「注册 / 凭据」报文体本就属于 Control 域
  （模型 `Control.Gateway.{Security,Supervision}`），因 `jumo-code generate` 当时对该模块 Blocked 才手写在本 crate。
  现按模型生成到 **`wist-control`**，请改用：
  `RegisterGateway` / `RenewGatewayCredential` / `VerifyGatewayCredential` / `GatewayEnrollmentResult` /
  `GatewayCredentialBundle` / `GatewayClientCertificate(+Status)` / `GatewayCredentialVerificationResult`
  （**`wist_control`**）。
  - **线上 JSON 不变**，但两处字段口径变化：`requested_at` / `issued_at` / `not_before` / `not_after`
    由 `String` 改为 `wist_control::DateTime`（同为 RFC3339 串），且生成类型**默认容忍未知字段**（不再 `deny_unknown_fields`）。
  - 消费方 `wist-center` / `wist-gwlinkd` 已同步切到 `wist-control`，且不再依赖本 crate。

## [0.6.0] - 2026-10-05

### 变更（不兼容）

- **`work` / `agent_uplink` 的 seam 报文移出到 `wist-api`**：`PollWork` / `WorkGrant` / `AckWork` /
  `WorkAccepted` / `ReportWorkResult` / `WorkResultAccepted`（+ kind 常量）改用 **`wist-api::work`**；
  `PollAgentUplink` / `AgentUplinkGrant` 改用 **`wist-api::agent_uplink`**（线上 JSON 不变）。
  **领域 / 状态**类（`WorkSpec*` / `WorkKind` / `StandingWork` / `OneShotWork` / 状态常量 /
  可执行性判定 / `AgentUplinkState`）仍留本 crate。

## [0.5.0] - 2026-10-05

### 变更（不兼容）

- **`gateway` 模块整体移出到 `wist-api`**：agent 面其余 seam 报文
  （action-plan / action-results / facts / discovery-policies）改用 **`wist-api::gateway`**
  （线上 JSON 不变，纯位置迁移）。`wist-contracts` 不再提供 `gateway` 模块。

## [0.4.0] - 2026-10-05

### 变更（不兼容）

- **`gateway` 模块里的 agent 面 seam 报文移出到 `wist-api`**：`AgentStatusReport`、`AgentStatusAck`、
  `AgentWorkState`、`AgentWorkStateChange`、`AgentCertificateStatus`、`AgentCredentialRenewal`
  请改用 **`wist-api::agent_status`**（线上 JSON 不变，纯位置迁移）。`wist_contracts::gateway`
  继续保留 action-plan / action-results / facts / discovery-policies 等其余报文。

## [0.3.0] - 2026-10-05

### 变更（不兼容）

- **agent 注册/续期 seam 报文迁到独立 crate `wist-api`**：`wist_contracts::enrollment` 不再提供
  `EnrollmentRequest` / `EnrollmentEnvelope` / `EnrollmentOutcome` / `EnrollmentStatus` /
  `CredentialRenewal` / `CredentialRenewed` 及 kind 常量 —— 请改用 `wist-api::enrollment`。
  这是「同一条 seam 两侧共用一份报文定义」的收口，避免两侧各留一份而静默漂移。
- `wist_contracts::enrollment` **保留**被多条 seam 复用的领域类型：`HostProfile`、`AgentIdentity`、
  `AgentIdentityStatus`、`CredentialBundle`、`InitialConfig`、`PolicyBinding`。

## [0.2.0] - 2026-10-04

### 变更（不兼容）

- **网关 ↔ 中心注册/凭据契约改为客户端证书（mTLS）**：
  - `GatewayCredentialBundle` 去掉 `auth_scheme` / `bearer_token` / `expires_at`，
    改为只能携带**客户端证书**：新增必填 `certificate`，以及可选 `ca_bundle` / `not_before` / `not_after`；
    `instance_id` 变为可选。
  - `RegisterGateway`：`csr_pem` → `certificate_signing_request`（**必填**）——mTLS 是唯一凭据路径。
  - `RenewGatewayCredential`：改为**证书轮换**（`current_certificate_serial` + `certificate_signing_request`）。
  - `VerifyGatewayCredential`：改为 `certificate_serial`。

### 新增

- **`GatewayClientCertificate` / `GatewayClientCertificateStatus`**：中心用 CA-G 签发的
  「每网关一张」客户端证书实体（绑定 `gateway_id`、可轮换、可单点吊销）。
- **`GatewayCredentialVerificationResult`**：以 `certificate_serial` 取代 `credential_id`。

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
