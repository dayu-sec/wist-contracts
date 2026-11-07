//! Structured telemetry record contract types.

use serde::{Deserialize, Serialize};

use crate::SCHEMA_VERSION_V1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryRecord {
    pub schema_version: String,
    /// 生产者全局唯一 ID（复合键 `(agent, seq)`）；旧数据无此字段时反序列化为空串。
    #[serde(default)]
    pub agent_id: String,
    pub observed_at: String,
    /// 记录所属 input（spool/路由用），不进帧。
    pub input_id: String,
    pub source_path: String,
    /// 记录来自**哪个采集面**（模型 `variant CollectionFamily`，闭集）与**哪条目录单元**
    /// （如 `mac-launchd-service`）。
    ///
    /// 为什么要有它：正文规则没写时，`category` 恒为泛化的 `agent.log`，于是
    /// 「这条来自哪个面」在数据里**没有位置** —— 两个面（如 launchd 与 wifi）一起跑就分不出来。
    /// 空串 = 这条不是平台派活来的（本机运维手工加的输入），本身就是有用的信息。
    /// 旧数据（或旧 spool 文件）无此字段时反序列化为空串。
    #[serde(default)]
    pub family: String,
    #[serde(default)]
    pub unit: String,
    /// Kubernetes 容器日志的来源身份（可选，见 [`PodLogRef`]）。
    ///
    /// 为什么单开一个字段而不是并进 `container`：`container` 是 Docker 与 k8s **共用**的
    /// `name/image/id` 三元组，而 namespace / pod / 节点 / 真实文件路径只对 k8s 有意义。
    /// 合在一起会让 Docker 记录也拖一堆空字段。
    /// **加性字段**：旧 spool / 旧帧没有它，解成 `None`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pod_log: Option<PodLogRef>,
    pub body: String,
    pub file_offset: u64,
    pub file_offset_end: u64,
    /// per-`agent` 全局单调递增序号（去重主键），与 checkpoint 同次原子写；
    /// 旧数据无此字段时反序列化为 0。
    #[serde(default)]
    pub seq: u64,
    /// 容器来源（仅**纯 Docker 主机**的容器日志有值）：从同目录 `config.v2.json` 读。
    ///
    /// 为什么要有它：k8s 侧的 pod/ns/container 编码在**文件名**里，而 Docker 侧文件名只有
    /// 容器 id —— 不把名字带上，数据面就只能看到一串 64 位十六进制。
    /// **加性字段**：旧 spool / 旧帧没有它，解成 `None`；本机手工输入也没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<ContainerRef>,
}

/// 记录来自哪个容器。只收**事实**（名字 / 镜像 / id），不收标签全集。
///
/// `name` 是 Docker 的 `Name`（去掉前导 `/`，如 `mysql`）；`image` 是 `Config.Image`
/// 原样（如 `bitnami/mysql:8.0`）；`id` 是容器目录名（容器重建会变，名字不变）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerRef {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub id: String,
}

/// Kubernetes 容器日志的来源身份：从**日志路径**还原的事实（不是从正文读出来的）。
///
/// 还原依据是 kubelet 建的两条路径：
///
/// * 软链接 `/var/log/containers/<pod>_<ns>_<container>-<容器ID>.log`；
/// * 真实文件 `/var/log/pods/<ns>_<pod>_<podUID>/<container>/0.log`（软链接指向它）。
///
/// 各字段的口径：
///
/// * `namespace` / `pod_name` / `pod_uid` —— 身份；
/// * `service` —— **服务标识，取容器名**（同一个 pod 可以有多个容器，用 pod 名分不出是谁）；
/// * `node_name` —— 这台机器（pod 所在的节点）；
/// * `log_path` —— **真实文件**路径（软链接解析后），可与 `file_offset` 对账；
/// * `container` —— 与 Docker 侧同一个三元组；`image` 要查容器运行时，采集端暂时查不到，
///   留空是可解释的事实（“没查到”），其余字段仍然齐。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PodLogRef {
    #[serde(default)]
    pub node_name: String,
    #[serde(default)]
    pub namespace: String,
    #[serde(default)]
    pub pod_name: String,
    #[serde(default)]
    pub pod_uid: String,
    #[serde(default)]
    pub service: String,
    #[serde(default)]
    pub log_path: String,
    #[serde(default)]
    pub container: ContainerRef,
}

impl TelemetryRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new_log(
        agent_id: String,
        observed_at: String,
        input_id: String,
        source_path: String,
        body: String,
        file_offset: u64,
        file_offset_end: u64,
        seq: u64,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_V1.to_string(),
            agent_id,
            observed_at,
            input_id,
            source_path,
            family: String::new(),
            unit: String::new(),
            pod_log: None,
            body,
            file_offset,
            file_offset_end,
            seq,
            container: None,
        }
    }

    /// 带上容器身份（仅纯 Docker 主机的容器日志）：它会让数据面能把记录分派到具体的
    /// 业务 / 网关 / 数据库容器，而不是一锅端。
    ///
    /// 用链式而不是给 `new_log` 再加参数：那里已经 8 个位置参数，而且这个字段
    /// 大多数来源根本没有。
    pub fn with_container(mut self, container: Option<ContainerRef>) -> Self {
        self.container = container;
        self
    }

    /// 带上 k8s 容器日志的来源身份（仅 `/var/log/containers` 那类路径有值）。
    ///
    /// 用链式而不是给 `new_log` 再加一组 `String`：那里已经 8 个位置参数，
    /// 再加六个同类字符串，谁都能把 namespace 与 pod 填反。
    pub fn with_pod_log(mut self, pod_log: Option<PodLogRef>) -> Self {
        self.pod_log = pod_log;
        self
    }

    /// 带上**来源身份**（采集面 + 目录单元）：会进数据帧，用于「这条来自哪个面」。
    ///
    /// 用链式而不是给 `new_log` 再加两个 `String` 参数：那里已经 8 个位置参数，
    /// 全是 `String`，再加两个谁都能把面名和单元名填反。
    pub fn with_origin(mut self, family: impl Into<String>, unit: impl Into<String>) -> Self {
        self.family = family.into();
        self.unit = unit.into();
        self
    }
}

/// 数据帧信封（数据平面 TCP 帧的 JSON 信封部分，短名）。
///
/// 帧完整形态为 `{envelope} LOGRAW: <正文>`；本结构只对应信封
/// `{schema, agent, ts, seq, command_id?, family?, unit?, node_name?, namespace?, pod_name?,
/// pod_uid?, service?, log_path?, container?}`，正文不进 JSON、不转义。
/// 字段用 `#[serde(rename)]` 映射线上短名；`agent` 向后兼容（缺省为空串）。
///
/// `family` / `unit`：**采集面**（闭集，见 `doc/design/center/collection-families.md`）
/// 与目录单元 id。**空则不出现在帧里** —— 这样本机手工输入与旧帧逐字节不变，
/// 数据面用 `opt(...)` 读也就有了明确语义（读不到 = 不是派活来的）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataFrame {
    #[serde(rename = "schema")]
    pub schema_version: String,
    #[serde(rename = "agent", default)]
    pub agent_id: String,
    #[serde(rename = "ts")]
    pub observed_at: String,
    pub seq: u64,
    /// 调查报告（` IVSREP: ` 帧）的**命令 id**：与 ack 走控制面，两者靠它 join；
    /// 计算区证据服务也拿它当幂等键（`cmd_id`）。
    ///
    /// 为什么进信封、而不是留给数据面从正文里取：正文是**原样字符串**，
    /// 数据面（WPL/OML）没有「保留原文 + 解析 JSON」的能力 —— 用 `json(...)` 解正文
    /// 会把 `body` 原文弄丢，而网关「主机 → 调查」页读的正是 `body`。
    ///
    /// 只有这类帧带值；空则**不出现在帧里**（与 `family`/`unit` 同口径：手工输入
    /// 与旧帧逐字节不变）。与 `body.command_id` 同值，两份都是为了各自消费者不用再解一层。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub command_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub family: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub unit: String,
    /// k8s 容器日志的来源身份（见 [`PodLogRef`]）。平铺进信封：数据面按 `namespace` /
    /// `service` 分派，不该再多解一层。空则**不出现在帧里**（与 `family`/`unit` 同口径）。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub node_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub namespace: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pod_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pod_uid: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub service: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub log_path: String,
    /// 容器身份（可选，见 [`ContainerRef`]。空则**不出现在帧里**——与 `family`/`unit`
    /// 同口径：手工输入与旧帧的字节不变。
    ///
    /// k8s 来源的这个值来自 [`PodLogRef::container`]：同一条记录**只有一份**容器身份。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<ContainerRef>,
}

impl From<&TelemetryRecord> for DataFrame {
    fn from(record: &TelemetryRecord) -> Self {
        Self {
            schema_version: record.schema_version.clone(),
            agent_id: record.agent_id.clone(),
            observed_at: record.observed_at.clone(),
            seq: record.seq,
            // 日志/事实/快照帧没有命令 id：只有 ` IVSREP: ` 帧（由 `DataFrame::new`
            // 构造后显式赋值）才带。
            command_id: String::new(),
            family: record.family.clone(),
            unit: record.unit.clone(),
            node_name: record
                .pod_log
                .as_ref()
                .map(|pod| pod.node_name.clone())
                .unwrap_or_default(),
            namespace: record
                .pod_log
                .as_ref()
                .map(|pod| pod.namespace.clone())
                .unwrap_or_default(),
            pod_name: record
                .pod_log
                .as_ref()
                .map(|pod| pod.pod_name.clone())
                .unwrap_or_default(),
            pod_uid: record
                .pod_log
                .as_ref()
                .map(|pod| pod.pod_uid.clone())
                .unwrap_or_default(),
            service: record
                .pod_log
                .as_ref()
                .map(|pod| pod.service.clone())
                .unwrap_or_default(),
            log_path: record
                .pod_log
                .as_ref()
                .map(|pod| pod.log_path.clone())
                .unwrap_or_default(),
            // 同一条记录只有一份容器身份：k8s 来源那份在 `pod_log` 里。
            container: record
                .container
                .clone()
                .or_else(|| record.pod_log.as_ref().map(|pod| pod.container.clone())),
        }
    }
}

impl DataFrame {
    /// 新建一帧数据帧信封（信号无关，`schema` 固定为 v1）。
    ///
    /// 用于无 `TelemetryRecord` 的信号（如指标），调用方自定 `seq`。
    pub fn new(agent_id: impl Into<String>, observed_at: impl Into<String>, seq: u64) -> Self {
        Self {
            schema_version: SCHEMA_VERSION_V1.to_string(),
            agent_id: agent_id.into(),
            observed_at: observed_at.into(),
            seq,
            command_id: String::new(),
            family: String::new(),
            unit: String::new(),
            node_name: String::new(),
            namespace: String::new(),
            pod_name: String::new(),
            pod_uid: String::new(),
            service: String::new(),
            log_path: String::new(),
            container: None,
        }
    }

    /// 带上命令 id（仅调查报告 ` IVSREP: ` 帧用）。
    ///
    /// 用链式而不是给 `new` 再加一个参数：`new` 是信号无关的，命令 id 只有调查这条
    /// 通道有；而且它必须在**发帧前**就确定（控制面 ack 用同一个值 join）。
    pub fn with_command_id(mut self, command_id: impl Into<String>) -> Self {
        self.command_id = command_id.into();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{ContainerRef, DataFrame, TelemetryRecord};

    fn record() -> TelemetryRecord {
        TelemetryRecord::new_log(
            "agent-001".to_string(),
            "2026-04-14T00:00:00Z".to_string(),
            "input-a".to_string(),
            "/tmp/app.log".to_string(),
            "raw line".to_string(),
            0,
            8,
            7,
        )
    }

    #[test]
    fn round_trips_json() {
        let record = record();
        let encoded = serde_json::to_string(&record).expect("serialize");
        let decoded: TelemetryRecord = serde_json::from_str(&encoded).expect("deserialize");
        assert_eq!(decoded, record);
        assert_eq!(decoded.agent_id, "agent-001");
        assert_eq!(decoded.schema_version, "v1");
        assert_eq!(decoded.seq, 7);
    }

    #[test]
    fn old_record_without_agent_id_and_seq_deserializes() {
        // 旧数据：无 agent_id、无 seq，应退化为空串 / 0（#[serde(default)] 向后兼容）。
        let json = r#"{"schema_version":"v1","observed_at":"2026-04-14T00:00:00Z","input_id":"app","source_path":"/tmp/app.log","body":"raw","file_offset":0,"file_offset_end":8}"#;
        let decoded: TelemetryRecord = serde_json::from_str(json).expect("deserialize");
        assert_eq!(decoded.agent_id, "");
        assert_eq!(decoded.seq, 0);
        assert_eq!(decoded.input_id, "app");
    }

    #[test]
    fn rejects_unknown_fields() {
        let json = r#"{"schema_version":"v1","agent_id":"a","observed_at":"t","input_id":"i","source_path":"p","body":"b","file_offset":0,"file_offset_end":1,"seq":0,"extra":true}"#;
        assert!(serde_json::from_str::<TelemetryRecord>(json).is_err());
    }

    #[test]
    fn data_frame_serializes_with_short_names() {
        let frame = DataFrame::from(&record());
        assert_eq!(
            serde_json::to_string(&frame).expect("serialize"),
            r#"{"schema":"v1","agent":"agent-001","ts":"2026-04-14T00:00:00Z","seq":7}"#
        );
    }

    #[test]
    fn data_frame_carries_the_origin_and_omits_it_when_unknown() {
        // 空的面/单元**不出现这个键**：本机手工输入与旧帧逐字节不变，
        // 数据面用 `opt(...)` 读也就有明确语义（读不到 = 不是派活来的）。
        let pulled = record().with_origin("ServiceLifecycle", "mac-launchd-service");
        assert_eq!(
            serde_json::to_string(&DataFrame::from(&pulled)).expect("serialize"),
            r#"{"schema":"v1","agent":"agent-001","ts":"2026-04-14T00:00:00Z","seq":7,"family":"ServiceLifecycle","unit":"mac-launchd-service"}"#
        );
        // 旧帧（没有这两个键）仍能反序列化。
        let old: DataFrame =
            serde_json::from_str(r#"{"schema":"v1","agent":"a","ts":"t","seq":1}"#)
                .expect("deserialize");
        assert_eq!(old.family, "");
        assert_eq!(old.unit, "");
    }

    #[test]
    fn data_frame_from_record_maps_fields() {
        let record = record().with_origin("NetworkFirewall", "mac-network-wifi");
        let frame = DataFrame::from(&record);
        assert_eq!(frame.schema_version, record.schema_version);
        assert_eq!(frame.agent_id, record.agent_id);
        assert_eq!(frame.observed_at, record.observed_at);
        assert_eq!(frame.seq, record.seq);
        assert_eq!(frame.family, "NetworkFirewall");
        assert_eq!(frame.unit, "mac-network-wifi");
    }

    #[test]
    fn container_identity_round_trips_and_stays_out_of_the_frame_when_absent() {
        let plain = record();
        assert!(
            !serde_json::to_string(&DataFrame::from(&plain))
                .expect("serialize")
                .contains("container"),
            "没有容器身份时不应该多出一个键"
        );

        let tagged = record().with_container(Some(ContainerRef {
            name: "mysql".to_string(),
            image: "bitnami/mysql:8.0".to_string(),
            id: "0123456789abcdef".to_string(),
        }));
        let json = serde_json::to_string(&DataFrame::from(&tagged)).expect("serialize");
        assert!(json.contains(r#""container":{"name":"mysql","image":"bitnami/mysql:8.0","id":"0123456789abcdef"}"#), "{json}");

        // spool 里的记录也要能读回来（重启重放场景）。
        let back: TelemetryRecord =
            serde_json::from_str(&serde_json::to_string(&tagged).expect("serialize record"))
                .expect("deserialize");
        assert_eq!(back.container, tagged.container);

        // 旧帧 / 旧 spool 没有这个键：解成 `None`，不能报错。
        let old: TelemetryRecord = serde_json::from_str(
            r#"{"schema_version":"v1","agent_id":"a","observed_at":"t","input_id":"i","source_path":"p","body":"b","file_offset":0,"file_offset_end":1,"seq":0}"#,
        )
        .expect("deserialize");
        assert_eq!(old.container, None);
    }

    #[test]
    fn a_record_without_origin_still_round_trips() {
        // 旧 spool 文件里的记录没有这两个字段：读出来是空串，不能报错。
        let json = r#"{"schema_version":"v1","agent_id":"a","observed_at":"t","input_id":"i","source_path":"p","body":"b","file_offset":0,"file_offset_end":1,"seq":0}"#;
        let decoded: TelemetryRecord = serde_json::from_str(json).expect("deserialize");
        assert_eq!(decoded.family, "");
        assert_eq!(decoded.unit, "");
    }

    #[test]
    fn pod_log_identity_lands_flat_in_the_frame_and_round_trips_in_the_record() {
        let pod = super::PodLogRef {
            node_name: "ent-bas-warpaxis-01".to_string(),
            namespace: "default".to_string(),
            pod_name: "doriscluster-helm-ms-2".to_string(),
            pod_uid: "98af449f-af1b-4a19-8f82-f5f82a198794".to_string(),
            service: "metaservice".to_string(),
            log_path: "/var/log/pods/default_doriscluster-helm-ms-2_98af449f-af1b-4a19-8f82-f5f82a198794/metaservice/0.log".to_string(),
            container: ContainerRef {
                name: "metaservice".to_string(),
                // 镜像要查容器运行时：查不到就是空串，字段仍在。
                image: String::new(),
                id: "ca018a1ea97ad12934aa236df87fc3c2816ea70c41a07134b1f3298c9d73e217"
                    .to_string(),
            },
        };
        let tagged = record()
            .with_origin("ContainerWorkload", "linux-container-stdout")
            .with_pod_log(Some(pod.clone()));

        // 线上信封就是 issue 里那个形状：身份平铺，`container` 只有一份。
        assert_eq!(
            serde_json::to_string(&DataFrame::from(&tagged)).expect("serialize"),
            concat!(
                r##"{"schema":"v1","agent":"agent-001","ts":"2026-04-14T00:00:00Z","seq":7"##,
                r##","family":"ContainerWorkload","unit":"linux-container-stdout""##,
                r##","node_name":"ent-bas-warpaxis-01","namespace":"default""##,
                r##","pod_name":"doriscluster-helm-ms-2""##,
                r##","pod_uid":"98af449f-af1b-4a19-8f82-f5f82a198794""##,
                r##","service":"metaservice""##,
                r##","log_path":"/var/log/pods/default_doriscluster-helm-ms-2_98af449f-af1b-4a19-8f82-f5f82a198794/metaservice/0.log""##,
                r##","container":{"name":"metaservice","image":"","id":"ca018a1ea97ad12934aa236df87fc3c2816ea70c41a07134b1f3298c9d73e217"}}"##
            )
        );

        // spool 里也要能读回来（重启重放），且不带 `container` 以外的多余键。
        let back: TelemetryRecord =
            serde_json::from_str(&serde_json::to_string(&tagged).expect("serialize record"))
                .expect("deserialize");
        assert_eq!(back.pod_log, Some(pod));
        assert_eq!(back.container, None);
    }

    #[test]
    fn a_docker_record_keeps_a_single_container_identity() {
        // Docker 来源没有 pod_log：帧里不应多出 k8s 的身份键。
        let tagged = record().with_container(Some(ContainerRef {
            name: "mysql".to_string(),
            image: "bitnami/mysql:8.0".to_string(),
            id: "0123".to_string(),
        }));
        let frame = DataFrame::from(&tagged);
        assert_eq!(frame.container, tagged.container);
        assert_eq!(frame.namespace, "");
        assert_eq!(frame.log_path, "");
        let json = serde_json::to_string(&frame).expect("serialize");
        assert!(!json.contains("namespace"), "{json}");
        assert!(!json.contains("log_path"), "{json}");
    }

    #[test]
    fn a_new_frame_round_trips_back_into_the_same_identity() {
        // 数据面 / 排障工具可能把帧再解回来：平铺的键必须能原样收回。
        let pod = super::PodLogRef {
            node_name: "n1".to_string(),
            namespace: "ns".to_string(),
            pod_name: "pod".to_string(),
            pod_uid: "uid".to_string(),
            service: "svc".to_string(),
            log_path: "/var/log/pods/ns_pod_uid/svc/0.log".to_string(),
            container: ContainerRef {
                name: "svc".to_string(),
                image: String::new(),
                id: "abc".to_string(),
            },
        };
        let frame = DataFrame::from(&record().with_pod_log(Some(pod)));
        let back: DataFrame =
            serde_json::from_str(&serde_json::to_string(&frame).expect("serialize"))
                .expect("deserialize");
        assert_eq!(back, frame);
        assert_eq!(back.namespace, "ns");
        assert_eq!(back.log_path, "/var/log/pods/ns_pod_uid/svc/0.log");
        assert_eq!(back.container.map(|c| c.id), Some("abc".to_string()));
    }

    #[test]
    fn an_old_frame_without_the_identity_keys_decodes_to_empty_strings() {
        // 旧数据面 / 旧帧没有这些键：空串 = “不知道”，不能报错。
        let frame: DataFrame = serde_json::from_str(
            r#"{"schema":"v1","agent":"a","ts":"t","seq":1,"family":"ContainerWorkload","unit":"linux-container-stdout"}"#,
        )
        .expect("deserialize");
        assert_eq!(frame.namespace, "");
        assert_eq!(frame.pod_name, "");
        assert_eq!(frame.pod_uid, "");
        assert_eq!(frame.service, "");
        assert_eq!(frame.log_path, "");
        assert_eq!(frame.node_name, "");
        assert_eq!(frame.container, None);
        // 旧帧没有该键：解出来是空串，不是报错（升级后重放不能挂）。
        assert_eq!(frame.command_id, "");
    }

    #[test]
    fn an_old_spool_record_without_pod_log_decodes_to_none() {
        // 旧 spool 文件里的记录没有 `pod_log`：升级后重放不得失败。
        let json = r#"{"schema_version":"v1","agent_id":"a","observed_at":"t","input_id":"i","source_path":"p","family":"ContainerWorkload","unit":"linux-container-stdout","body":"b","file_offset":0,"file_offset_end":1,"seq":0}"#;
        let decoded: TelemetryRecord = serde_json::from_str(json).expect("deserialize");
        assert_eq!(decoded.pod_log, None);
        assert_eq!(decoded.container, None);
    }

    #[test]
    fn pod_log_ref_tolerates_missing_optional_keys() {
        // 只有名字 / 命名空间也能解（其它字段为空串）—— 生产者可能只拼得出前面几个。
        let pod: super::PodLogRef =
            serde_json::from_str(r#"{"namespace":"ns","pod_name":"pod"}"#).expect("deserialize");
        assert_eq!(pod.namespace, "ns");
        assert_eq!(pod.service, "");
        assert_eq!(pod.container, ContainerRef::default());
    }

    #[test]
    fn data_frame_deserializes_missing_agent_as_empty() {
        let json = r#"{"schema":"v1","ts":"2026-04-14T00:00:00Z","seq":7}"#;
        let frame: DataFrame = serde_json::from_str(json).expect("deserialize");
        assert_eq!(frame.agent_id, "");
        assert_eq!(frame.seq, 7);
    }

    #[test]
    fn data_frame_omits_empty_command_id() {
        // 空值不出现在帧里：手工输入与旧帧（无该键）逐字节不变。
        let json = serde_json::to_string(&DataFrame::new("a", "t", 1)).expect("serialize");
        assert!(!json.contains("command_id"), "{json}");
    }

    #[test]
    fn data_frame_round_trips_command_id() {
        // 调查报告帧：必须带上命令 id（ack 走控制面、正文走数据面，两边靠它 join）。
        let frame = DataFrame::new("a", "t", 1).with_command_id("cmd_agent-1_refresh_1");
        let json = serde_json::to_string(&frame).expect("serialize");
        assert!(
            json.contains(r#""command_id":"cmd_agent-1_refresh_1""#),
            "{json}"
        );
        let back: DataFrame = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.command_id, "cmd_agent-1_refresh_1");
    }

    #[test]
    fn a_record_backed_frame_never_carries_a_command_id() {
        // 日志 / 事实 / 快照帧都由 `TelemetryRecord` 派生：它们没有命令 id，
        // 不能因为信封上有这个字段就凭空造一个（否则下游会把日志当成调查产物）。
        let frame = DataFrame::from(&record());
        assert_eq!(frame.command_id, "");
        let json = serde_json::to_string(&frame).expect("serialize");
        assert!(!json.contains("command_id"), "{json}");
    }
}
