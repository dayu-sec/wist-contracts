# 更新日志

本文件记录 `wist-contracts` 的所有重要变更。格式遵循 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)，
版本号遵循[语义化版本](https://semver.org/lang/zh-CN/)。

## [0.15.0] - 2026-11-26

### 新增（采集来源 kind `HostFlow`）

- `CollectionSourceKind::HostFlow`：主机侧流记录（对外网卡的 TCP/UDP/ICMP 五元组窗口聚合）。
  载体是 tc + eBPF，设计见 `wist-agentd/docs/design/host-ebpf-flow-collection.md`。
- `target` **只认空或 `auto`**：观测哪张网卡是**本机事实**，平台指定不出来。写死接口名
  （`eth0`）一律不可执行 —— 与自探域共用 `accepts_target`，不在 `is_executable_source`
  里另写一份判定。
- `EXECUTABLE_SOURCE_KINDS` 追加 `HostFlow`。换载体（XDP / pod veth / conntrack）应新增
  kind，不复用本值：不同载体的覆盖边界与计数口径不同，混在一起会让数据无法解释。

### 兼容性

- `CollectionSourceKind` 新增变体：下游若对该枚举写了**穷尽** `match`，编译期会要求补齐；
  用 `matches!` / `accepts_target` 的调用点不需要改。`EXECUTABLE_SOURCE_KINDS` 是追加式增加。
- 发布顺序仍是**读者先发**：agentd / 网关先能认这个 kind，知识库再把对应单元从 `draft`
  翻 `active`。旧 agentd 遇到 `HostFlow` 的 `active` 单元会如实报 `unsupported`，不会静默。

## [0.14.0] - 2026-11-22

### 新增（执行计划的**签发证明** `ActionPlanAttestation`）

- `ActionPlan` 新增 `attestation: Option<ActionPlanAttestation>`（`#[serde(default, skip_serializing_if)]`）。
  它回答「谁批的、批的是不是这一份」；`None` = 未签发（E2 的直发路径）。
- `plan_digest(&ActionPlan) -> String`：`plan-v1:sha256:<hex>`。规范化靠「序列化成 Value →
  递归排序对象键 → 紧凑输出」，**不另写镜像结构体** —— 后者一旦忘同步新字段，新字段就
  不在摘要范围内，那是静默的安全洞。`attestation` 自身被显式排除（它是签名载体）。
- `canonical_json(&Value) -> String`：签 / 转发 / 验三端必须算出**同一串字节**。
- `ActionPlanAttestation`：`plan_digest` + `approval` + `risk_level` + `issued_by` + `key_id`
  + `algorithm` + `signed_at` + `expires_at` + `signature`；`signing_payload()` = 去掉
  `signature` 后的规范 JSON（新加字段自动进签名范围）。
- `ActionPlanApproval { approval_id, approvers, required_approvers, approved_at }`：
  **在签名覆盖范围内**，所以执行侧不必回头问中心也能判断「够不够人」。

### 兼容性

- `attestation` 是加性可选字段，旧计划仍可解析；
- 但 `ActionPlan` 是 `deny_unknown_fields`，**旧读取方会整份拒收**带 attestation 的计划 →
  发布序仍是「读者先发」：agentd 先能解，网关 / 中心再签；
- 摘要前缀是 `plan-v1`：形状变了要升前缀，不要悄悄改（黄金向量在测试里）。
- `plan_digest` 对 `args: null` 与「没有 args」**归一到同一个摘要**：两者在执行侧等价
  （`unwrap_or(&null)`），否则一次 JSON 往返就能让同一份计划被判“内容被改过”。
  对不带 `null` args 的计划摘要不变（黄金向量未动）。

## [0.13.0] - 2026-11-22

### 新增（`ActionPlanStep.args`：执行计划带参数）

- `ActionPlanStep` 新增 `args: Option<serde_json::Value>`（`#[serde(default, skip_serializing_if)]`）：
  `kind = invoke` 的参数，缺省 = 无参数。形状由各 opcode 在**执行侧**校验，
  契约只约束「JSON 对象或 `null`」（`wist-validate` 的 `invalid_step_args`）。
- 为什么用 `Value` 而不是强类型：`disk.cleanup.*` 与 `process.list` 没有共同字段，
  强类型会逼出一堆互不相干的入参 struct；契约层保持中性，参数 schema 归 opcode 注册表。

### 新增（`ActionPlanConstraints.allow`：路径白名单）

- 新增 `ActionPlanAllow { paths: Vec<String> }` 与 `constraints.allow`（`#[serde(default)]`）。
  需要调用方指定路径的 op（`file.stat` / `file.read_range` …）必须先过这一层。
- **默认拒绝**：`None` / 空列表 = 什么都不许读 —— 不是“什么都可以”。
- 路径比较在**执行侧**做（canonicalize + 前缀匹配），契约只存字符串。

### 兼容性

- 加性字段 + `default`：旧计划（无 `args`）仍可解析。
- 但 `ActionPlanProgram` / `ActionPlanStep` 是 `deny_unknown_fields`，**旧读取方会整份拒收**
  带 `args` 的新计划 → 发布序必须是「读者先发」：agentd 先能解，中心再产出。
- 新增公共字段会破坏下游的结构体字面量，需要补 `args: None`（本批已同步 agentd / validate）。

## [0.12.0] - 2026-11-21

### 新增（关键进程指标的采集来源）

- `CollectionSourceKind::KeyProcessMetrics`（第 9 类来源）：`target` 是**匹配签名列表**，
  形如 `unit:kafka|port:9092|exe:kafka`。具体选哪些进程由 agentd 本机判定 ——
  与 Kafka / MySQL / PG / Doris「端点/目标只来自自探」同一条纪律：授权只说**采不采**，
  不指定主机上的具体对象。
- `parse_key_process_target()` + `KeyProcessSignature` / `KeyProcessSignaturePrefix` /
  `KeyProcessTargetError`，以及闭集 `KEY_PROCESS_SIGNATURE_PREFIXES`（`unit` / `exe` / `port` / `cmdline`）。
- `CollectionSourceKind::accepts_target()` 对 `KeyProcessMetrics` 走**签名校验**：
  未知前缀 / 空值 / 值里带空白 / `port` 越界一律拒；`auto` 在这里**不合法**
  （它只说“自己找”，没说采什么进程）。
- 两条在评审时补充的限制（都是“能构造出来但永远不会命中”的形态）：
  - `exe` 的值**不得含 `/`** —— 匹配按 basename 做，`exe:/usr/bin/nginx` 永远不命中；
  - `port` 必须是**纯十进制数字**且 1..=65535 —— `+9092` / `0x2384` 不接受
    （`u16::from_str` 会接受前导 `+`，所以不能只靠它）。
- `KeyProcessSignaturePrefix::ALL` 是前缀的**事实源**；`KEY_PROCESS_SIGNATURE_PREFIXES`
  降为展示用常量，两者由单测钉住（本仓不允许两份手抄清单）。
- 空前缀（`:kafka`）按**缺前缀**报错，不报“未知前缀”：报错要能告诉人该补什么。
- `parse_key_process_target` 带文档示例（doctest 跑在 `cargo test` 里）。

### 为什么

- 逐进程指标在 0.35.0 被停采（无授权来源 + 只能上报 3 条随机样本）。要恢复就必须先能
  “可解释地选”：由知识库枚举每类关键进程的签名，agentd 只做机械匹配。
- 匹配面（kafka 的 exe 其实是 `java`、裸跑 JVM 只能看 cmdline）是**策展知识**，
  写进知识库、走审定，不进代码、不进用户视野。
- 未知前缀必须拒：一份配错的授权如果被“跳过”，现场看到的是“授权了但没数据”，
  分不清是没装、配错、还是 agent 不支持。

### 兼容性

- 新增枚举变体，**下游对 `CollectionSourceKind` 做穷举 `match` 的代码需要补分支**
  （本仓以 0.12.0 发布，不再向后兼容）。
- 旧网关 / 旧 agentd 看到 `KeyProcessMetrics` 会当未知 kind 处理（`parse` 返回 `None`），
  不会误采；新来源真正生效需 gateway 与 agentd 同步升级。

## [0.11.0] - 2026-11-10

### 变更（采集结果改多字段正交模型）

- **`AgentLocalCollection` 不再用单一 `status` 表达采集结果**，拆成三个正交字段：
  - `prerequisites`：`ready` / `blocked` —— 前置条件（工具 / 依赖 / 来源 / 目标）是否具备；
  - `result`：`success` / `partial` / `failure` / `cancelled` / `pending` —— 执行结果；
  - `reasons`：`[{kind, subject, detail}]` —— 为什么不是成功；不参与状态判定。
- 新增 `CollectionReason`，以及三个闭集模块 `collection_prerequisites` / `collection_result` /
  `collection_reason_kind`（`missing_tool` / `missing_dependency` / `source_not_ready` /
  `target_not_ready` / `permission` / `missing_input` / `run_failed` / `unsupported` /
  `truncated` / `invalid_spec` / `no_units` / `no_content` / `incomplete_data` / `paused` /
  `standby` / `metric_partial`）。

### 为什么

- 单一 `status` 把三件事挤在一个词里：**该不该采**、**能不能采**、**采成没采成**。
  结果是 `smartctl: 工具缺失（装上后自动恢复）` 被报成采集失败（页面红色），
  而它其实是前置条件不满足 —— 装上工具就自动恢复。

### 兼容性

- 旧字段 `status` / `detail` 保留（`serde(default)`），由新字段派生后继续输出，
  供未升级的网关 / 中心 / 页面使用；只报旧字段的报文读取时新字段为空。
- **新 agentd 的输出含新字段，旧网关（`deny_unknown_fields`）会拒收整份上报**：
  升级顺序必须是 gateway / center → agentd。
- 用结构体字面量构造 `AgentLocalCollection` 的下游需补字段。

## [0.10.4] - 2026-11-08

### 新增

- **采集面采集结果上报**：`AgentLocalStandingWork.collection`（`AgentLocalCollection`）——
  agentd 把每个采集面的本 tick 采集结果（`ok` / `degraded` / `failed` / `unknown` / `paused`
  + 原因 + 观测时间）随本机工作视图上报，网关透传，管理面「Agent 工作」卡上可直接看到
  「这个面到底采成没采成」。字段可选：旧 agentd 不报时不出现在报文里。
- `collection_status` 常量闭集，避免两侧各写一遍字符串。

## [0.10.3] - 2026-11-07

### 新增

- **外部系统的采集来源 kind 收进契约**：新增 `CollectionSourceKind`
  （`FileGlob` / `Exporter` / `UnifiedLogPredicate` / `MetricInterval`，
  以及 `KafkaCluster` / `MysqlInstance` / `PostgresInstance` / `DorisCluster`），
  配套 `capability::{COLLECT_LOGS, COLLECT_METRICS}` 与 `AUTO_TARGET`。
  后四个域的端点**只来自 agent 本机自探**：`accepts_target` 只认 `auto`（空也兼容旧网关），
  写具体地址一律不支持（不扩大 SSRF 面）。此前这些字符串只存在 agentd 内部，
  网关与 agentd 各写一份，存在漂移风险。

## [0.10.2] - 2026-11-06

### 新增

- **数据帧信封新增可选 `command_id`**（`DataFrame.command_id` / `DataFrame::with_command_id`）：
  调查报告（` IVSREP: ` 帧）把命令 id 同时放进**信封**。数据面（WPL/OML）解不了正文里的
  JSON，计算区证据服务又需要它做幂等键 —— 只写在正文里等于没给数据面。
  空值不出现在帧里，其它信号（日志 / 事实 / 快照）与旧帧的字节不变。

## [0.10.1] - 2026-11-05

### 新增

- **`PodLogRef` + 信封平铺身份**（`TelemetryRecord.pod_log`）：k8s 容器日志的来源身份
  从**日志路径**还原，不再只有一串容器 id。信封新增 `node_name` / `namespace` /
  `pod_name` / `pod_uid` / `service` / `log_path`（`container` 沿用同一个三元组，
  `image` 查不到时为空串）。`service` 取**容器名**：同一个 pod 可以有多个容器，用 pod 名分不出是谁。
  全部是加性字段且空则不出现在帧里 —— Docker 记录与旧帧的字节不变。
- **`MULTILINE_MODES` 增 `container_cri`**：按**应用自己的头部**（行首时间戳 /
  glog / `RuntimeLogger`）切记录，不拿 CRI 的 `F` 当边界（一次事务状态输出会被拆成很多条 `F` 行）。

## [0.10.0] - 2026-11-03

### 变更（不兼容）

- **`FactContent` 纳入高置信度证据**：新增 `service_units`（服务管理器单元，已归一）与
  `container_names`（容器名，k8s 下即 pod spec 的容器名）。它们是选择类 label `app` 的
  推断输入，**决定指标序列带不带 `app`**，所以必须进内容摘要 —— 证据变了而摘要不变，
  会让指标永远带着旧用途，且没有任何一层能发现。
- `FACT_DIGEST_VERSION`：`fact-v3` → **`fact-v4`**。升级后每台 agent 会强制重报一次
  （这正是版本前缀要的语义），两侧版本不一致时只会记一条 `FactDigestMismatch` 告警，不拒收。

### 新增

- `FactEvidence`：证据的独立结构体；`FactContent::new` 签名不变，
  需要证据时用 `with_evidence(...)` 链式补上（7 个位置参数再加两个就没人记得住顺序了）。

## [0.9.9] - 2026-10-30

### 新增

- **`capability_report::InvestigationCapabilities`**（调查能力）：`surfaces`（本机真实现的调查面）
  + `privileged`（是否 root/系统服务运行，决定同一道题给“完整答案”还是“部分答案”）。
  与 `LogsCapabilities` 同一条纪律：网关据此**在入队前**判断能不能做，而不是发下去换一个
  `unsupported`。

### 文档 / 测试

- 说明**为什么单开一个桶**而不并进 `LogsCapabilities`：调查面与采集面是两条授权轴
  （见 `doc/design/center/investigation-surfaces.md`），“能不能采这类事件”与“能不能查这个
  问题”不能互相冒充。
- 新增测试：空对象可解（旧 agent / 最小报文）、往返稳定、字段拼错要响。

## [0.9.8] - 2026-10-24

### 新增

- **`work::is_container_log_target(target)`**：容器日志来源的**形状判据**（k8s 与 Docker 共用）。
  判据只要求路径里有一个 `containers` 段 —— Docker 可以改 `--data-root`，写死 `/var/lib/docker`
  会误拒现场；同时接受目录形态（`…/containers`），不再把显式目录来源错拒。
  网关装载校验与 agentd 本机配置校验共用这一份，避免两处口径漂移。

### 文档 / 测试

- `ContainerFilters` 补上**组合语义**：维度内 OR、维度间 AND，`*_not_in` 命中即不收，
  同值同时出现在 `name_in` / `name_not_in` 时排除优先。
- 订正 `name_not_in` 的注释：排除式**不**要求“证明确实是它”（之前写反了）。
- `CONTAINER_FILTERS` 的措辞从“agentd 能承接的全部”改为“协议定义的词表” ——
  上报方必须**显式**列维度，不能直接抄这个常量（否则“契约加了一维”会自动变成“我也支持”）。
- 新增表驱动测试：只填一维时，`is_empty` / 序列化键名 / `used_dimensions()` 三者必须同时
  只反映该维；四维全空时整体不序列化。`validate` 改用 `FILTER_*` 常量，不再手抄字段名。

## [0.9.7] - 2026-10-24

### 新增（容器名过滤）

- **`ContainerFilters.name_in` / `name_not_in`**：按**容器名**选源 / 排除。

  两类载体上的含义：
  - k8s（`<pod>_<ns>_<container>-<id>.log`）：文件名里的 `<container>` 段；
  - 纯 Docker（`<root>/containers/<id>/<id>-json.log`）：文件名里**没有名字**，
    由采集端从同目录的 `config.v2.json` 取（与帧上的 `container.name` 同源）。

  能力轴自动扩展为 `filters:name_in` / `filters:name_not_in`：旧 agentd 不声明时，
  网关**裁掉带该过滤的来源**（而不是整份拒收）。

  语义细节（两端同一口径）：**包含式**过滤要求“证明确实是它”——取不到身份就不命中；
  **排除式**相反——取不到身份就**保留**（宁可多采，不静默丢）。

## [0.9.6] - 2026-10-24

### 新增（容器身份进帧）

- **`ContainerRef { name, image, id }`**，以及 `TelemetryRecord.container` /
  `DataFrame.container`（可选；**空则不出现在帧里**）。

  为什么需要：纯 Docker 主机的容器日志在文件名里只有容器 id，名字与镜像只存在于同目录的
  `config.v2.json`。不带它，数据面只能看到一串十六进制，无法把记录分派到具体的
  业务 / 网关 / 数据库容器（k8s 侧这个信息编码在文件名里，Docker 侧没有）。

  加性字段：旧 spool / 旧帧没有它 → `None`；本机手工输入也没有。不改既有字段语义，
  也不需要新能力轴（网关不下发它；数据面读不到就按“不知道哪个容器”处理）。

### 验证

- 真实数据面（`wparse` + `agent_uplink` 模型）实测：帧信封多出 `container` 键时仍正常解析
  （正文完整落盘，不落 miss）—— 加字段不会伤害旧数据面的解析。

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
