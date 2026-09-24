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
    pub body: String,
    pub file_offset: u64,
    pub file_offset_end: u64,
    /// per-`agent` 全局单调递增序号（去重主键），与 checkpoint 同次原子写；
    /// 旧数据无此字段时反序列化为 0。
    #[serde(default)]
    pub seq: u64,
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
            body,
            file_offset,
            file_offset_end,
            seq,
        }
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
/// `{schema, agent, ts, seq, family?, unit?}`，正文不进 JSON、不转义。
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
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub family: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub unit: String,
}

impl From<&TelemetryRecord> for DataFrame {
    fn from(record: &TelemetryRecord) -> Self {
        Self {
            schema_version: record.schema_version.clone(),
            agent_id: record.agent_id.clone(),
            observed_at: record.observed_at.clone(),
            seq: record.seq,
            family: record.family.clone(),
            unit: record.unit.clone(),
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
            family: String::new(),
            unit: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DataFrame, TelemetryRecord};

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
    fn a_record_without_origin_still_round_trips() {
        // 旧 spool 文件里的记录没有这两个字段：读出来是空串，不能报错。
        let json = r#"{"schema_version":"v1","agent_id":"a","observed_at":"t","input_id":"i","source_path":"p","body":"b","file_offset":0,"file_offset_end":1,"seq":0}"#;
        let decoded: TelemetryRecord = serde_json::from_str(json).expect("deserialize");
        assert_eq!(decoded.family, "");
        assert_eq!(decoded.unit, "");
    }

    #[test]
    fn data_frame_deserializes_missing_agent_as_empty() {
        let json = r#"{"schema":"v1","ts":"2026-04-14T00:00:00Z","seq":7}"#;
        let frame: DataFrame = serde_json::from_str(json).expect("deserialize");
        assert_eq!(frame.agent_id, "");
        assert_eq!(frame.seq, 7);
    }
}
