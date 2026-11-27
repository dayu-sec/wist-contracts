//! 主机流记录（` FLOW: ` 帧的正文）：`wist.host_flow/v1`。
//!
//! 为什么在契约 crate 里：三侧对同一个形状 —— agentd **产出**、数据面只搬不建模
//! （WPL/OML 只保留 `body` 原文）、平台负责富化与判决 —— 形状只应有一处定义
//! （与 [`crate::fact_summary`] / [`crate::telemetry_record`] 同一纪律）。
//!
//! 三层边界（设计见 `wist-agentd/docs/design/host-ebpf-flow-collection.md` §4）：
//! * 这里只放**观测事实 + 平台 join 键**；
//! * 任何推断（`action`、`needs_enrichment`）与云侧身份（`instance-id`/`vpc-id`/…）
//!   **一律不出现** —— 它们是第 ③ 层（平台富化）的事；
//! * `record_schema` 是**版本闸门**：加字段 = 升 `wist.host_flow/vN`，不悄悄改 v1。

use serde::{Deserialize, Serialize};

/// v1 记录版本。改字段语义要升它（不要悄悄改）。
pub const RECORD_SCHEMA_V1: &str = "wist.host_flow/v1";

/// 观测来源：主机侧 eBPF。下游据此与云侧 VPC 流日志**择一**使用，不相加。
pub const SOURCE_HOST_EBPF: &str = "host_ebpf";
/// 覆盖标记：主机视角 —— 看不到在到达网卡之前就被云侧策略丢弃的流量。
pub const COVERAGE_HOST_PARTIAL: &str = "host_partial";

/// 方向取值（与云侧词表一致，不是 `ingress`/`egress`）。
pub const DIRECTION_IN: &str = "in";
pub const DIRECTION_OUT: &str = "out";
pub const DIRECTIONS: &[&str] = &[DIRECTION_IN, DIRECTION_OUT];

/// 观测点：记录里保留它，排障时能一眼看出 hook 的位置。
pub const CAPTURE_TC_INGRESS: &str = "tc_ingress";
pub const CAPTURE_TC_EGRESS: &str = "tc_egress";
pub const CAPTURES: &[&str] = &[CAPTURE_TC_INGRESS, CAPTURE_TC_EGRESS];

/// 默认窗口长度（秒）。`partial = true` 时 `window_seconds` 是**实际覆盖**的秒数。
pub const WINDOW_SECONDS_DEFAULT: u32 = 60;

/// `partial_reason` 的闭集：窗口被提前截断时**必须**给出可归因的一句。
pub const PARTIAL_REASON_AGENT_START: &str = "agent_start";
pub const PARTIAL_REASON_PAUSED: &str = "paused";
pub const PARTIAL_REASON_INTERFACE_CHANGED: &str = "interface_changed";
pub const PARTIAL_REASON_SHUTDOWN: &str = "shutdown";
/// 墙钟跳变（NTP 回拨/手动改时间）：窗口归属靠「单调桶差 + 当前墙钟」换算，
/// 改时间会让映射失效 —— 那一窗按截断处理，下一窗重新锚定。
pub const PARTIAL_REASON_CLOCK_CHANGED: &str = "clock_changed";
pub const PARTIAL_REASONS: &[&str] = &[
    PARTIAL_REASON_AGENT_START,
    PARTIAL_REASON_PAUSED,
    PARTIAL_REASON_INTERFACE_CHANGED,
    PARTIAL_REASON_SHUTDOWN,
    PARTIAL_REASON_CLOCK_CHANGED,
];

/// `wire_bytes` 的来源。
///
/// `wire_len` 只有 5.14+ 的 `__sk_buff` 才有；取不到就整键不出现并写 `unavailable`
/// —— **不估算、不伪造精度**。
pub const WIRE_BYTES_SOURCE_WIRE_LEN: &str = "wire_len";
pub const WIRE_BYTES_SOURCE_UNAVAILABLE: &str = "unavailable";
pub const WIRE_BYTES_SOURCES: &[&str] =
    &[WIRE_BYTES_SOURCE_WIRE_LEN, WIRE_BYTES_SOURCE_UNAVAILABLE];

/// 一条窗口记录。
///
/// 字段与设计文档 §6 一一对应。`Option` 字段靠 `skip_serializing_if` **省掉**而不是写
/// `null`：下游能区分「没这个字段」（这个版本/这条记录没有）与「有值但为空」。
///
/// `deny_unknown_fields` 是**故意的**：v1 的字段集就是这一份，多一个键要么是拼错，
/// 要么是没升 `record_schema` 就加了字段 —— 两种都该在消费端当场炸掉，而不是继续读。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostFlowRecord {
    pub record_schema: String,
    pub source: String,
    pub coverage: String,
    pub capture: String,
    /// agent 本地稳定的采集实例标识（重启不变）：幂等键的第一段。
    pub collector_instance_id: String,
    pub host_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_name: Option<String>,
    pub kernel: String,
    pub ifname: String,
    pub ifindex: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub if_mac: Option<String>,
    /// 窗口开始时该网卡**可路由**地址的快照（已滤掉回环/链路本地/组播）。
    pub local_addrs: Vec<String>,
    pub direction: String,
    /// 协议号（IANA）：`6` TCP / `17` UDP / `1` ICMP / `58` ICMPv6。
    ///
    /// 字符串而非整数：与云侧词表/已有采集口径一致，下游不必为「有的源是数字、
    /// 有的是字符串」写两次转换。
    pub protocol: String,
    pub srcaddr: String,
    pub dstaddr: String,
    /// 端口也是字符串：ICMP 没有端口，固定 `"0"`（与云侧口径一致）。
    pub srcport: String,
    pub dstport: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icmp_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icmp_code: Option<String>,
    /// 分片/扩展头导致端口取不到：此时端口为 `"0"`，靠这个标记告诉下游「不是端口 0」。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_unavailable: Option<bool>,
    /// 窗口起点（Unix 秒，UTC 整分对齐）。
    pub start: u64,
    /// 窗口终点（半开区间：`[start, end)`）。
    pub end: u64,
    pub window_seconds: u32,
    /// Σ `gso_segs`：估算的 L3 报文数（不是「逐包相同」）。
    pub packets: u64,
    /// Σ `skb->len`：**L3** 字节（不含 L2），不是线缆字节。
    pub bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire_bytes: Option<u64>,
    pub wire_bytes_source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skb_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gso_skb_count: Option<u64>,
    /// 本窗口发生过聚合表上限淘汰。
    pub truncated: bool,
    /// 本窗口未能计入的事件数（表满、读表竞态、迟到落在已 flush 窗口）。
    pub dropped: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn record() -> HostFlowRecord {
        HostFlowRecord {
            record_schema: RECORD_SCHEMA_V1.to_string(),
            source: SOURCE_HOST_EBPF.to_string(),
            coverage: COVERAGE_HOST_PARTIAL.to_string(),
            capture: CAPTURE_TC_EGRESS.to_string(),
            collector_instance_id: "agent-host-d97e23da6111".to_string(),
            host_id: "host-d97e23da6111".to_string(),
            host_name: Some("vm-web-01".to_string()),
            kernel: "5.15.0-91-generic".to_string(),
            ifname: "eth0".to_string(),
            ifindex: 2,
            if_mac: Some("52:54:00:aa:bb:cc".to_string()),
            local_addrs: vec!["10.118.0.14".to_string()],
            direction: DIRECTION_OUT.to_string(),
            protocol: "6".to_string(),
            srcaddr: "10.118.0.14".to_string(),
            dstaddr: "88.214.25.121".to_string(),
            srcport: "50123".to_string(),
            dstport: "443".to_string(),
            icmp_type: None,
            icmp_code: None,
            port_unavailable: None,
            start: 1_789_469_760,
            end: 1_789_469_820,
            window_seconds: WINDOW_SECONDS_DEFAULT,
            packets: 13,
            bytes: 1460,
            wire_bytes: None,
            wire_bytes_source: WIRE_BYTES_SOURCE_UNAVAILABLE.to_string(),
            skb_count: Some(4),
            gso_skb_count: Some(1),
            truncated: false,
            dropped: 0,
            partial: None,
            partial_reason: None,
        }
    }

    /// 线上一共就这些键。多/少一个都是契约变更 —— 这条测试是唯一的口径。
    #[test]
    fn the_wire_field_set_is_exactly_the_documented_one() {
        let value = serde_json::to_value(record()).expect("serialize");
        let keys: BTreeSet<&str> = value
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        let expected: BTreeSet<&str> = [
            "record_schema",
            "source",
            "coverage",
            "capture",
            "collector_instance_id",
            "host_id",
            "host_name",
            "kernel",
            "ifname",
            "ifindex",
            "if_mac",
            "local_addrs",
            "direction",
            "protocol",
            "srcaddr",
            "dstaddr",
            "srcport",
            "dstport",
            "start",
            "end",
            "window_seconds",
            "packets",
            "bytes",
            "wire_bytes_source",
            "skb_count",
            "gso_skb_count",
            "truncated",
            "dropped",
        ]
        .into_iter()
        .collect();
        assert_eq!(keys, expected);
    }

    #[test]
    fn optional_fields_are_omitted_instead_of_null() {
        let text = serde_json::to_string(&record()).expect("serialize");
        for absent in [
            "icmp_type",
            "icmp_code",
            "port_unavailable",
            "wire_bytes",
            "partial",
            "partial_reason",
        ] {
            assert!(
                !text.contains(&format!("\"{absent}\"")),
                "`{absent}` 不该出现（省略而不是 null）：{text}"
            );
        }
        assert!(!text.contains("null"), "{text}");
    }

    #[test]
    fn a_record_roundtrips() {
        let original = record();
        let text = serde_json::to_string(&original).expect("serialize");
        let parsed: HostFlowRecord = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(parsed, original);
    }

    /// `deny_unknown_fields`：拼错键 / 未升版本就加字段，必须在消费端当场炸掉。
    #[test]
    fn unknown_keys_are_rejected() {
        let mut value = serde_json::to_value(record()).expect("serialize");
        value
            .as_object_mut()
            .expect("object")
            .insert("account_id".to_string(), "123".into());
        let err = serde_json::from_value::<HostFlowRecord>(value).expect_err("must reject");
        assert!(err.to_string().contains("account_id"), "{err}");
    }

    /// 云侧身份与判决字段**一律不出现**（设计 §6.6）。这条是防回归，不是文档。
    ///
    /// 按**键名**判，不按整串搜索：`collector_instance_id` 里含有 `instance_id` 子串，
    /// 那是 join 键，不是云侧身份。
    #[test]
    fn cloud_side_and_verdict_fields_never_appear() {
        let value = serde_json::to_value(record()).expect("serialize");
        let keys: BTreeSet<&str> = value
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        for forbidden in [
            "account-id",
            "account_id",
            "action",
            "az-id",
            "az_id",
            "cross-region",
            "cross_region",
            "instance-id",
            "instance_id",
            "interface-id",
            "interface_id",
            "log-status",
            "log_status",
            "region-id",
            "region_id",
            "subnet-id",
            "subnet_id",
            "needs_enrichment",
            "version",
        ] {
            assert!(
                !keys.contains(forbidden),
                "记录里不该有 {forbidden}：{keys:?}"
            );
        }
    }

    /// 词表是闭集：下游只会见到这几种取值。
    #[test]
    fn vocabularies_are_closed_sets() {
        assert_eq!(DIRECTIONS, &[DIRECTION_IN, DIRECTION_OUT]);
        assert_eq!(CAPTURES, &[CAPTURE_TC_INGRESS, CAPTURE_TC_EGRESS]);
        assert_eq!(WIRE_BYTES_SOURCES.len(), 2);
        assert_eq!(
            PARTIAL_REASONS,
            &[
                PARTIAL_REASON_AGENT_START,
                PARTIAL_REASON_PAUSED,
                PARTIAL_REASON_INTERFACE_CHANGED,
                PARTIAL_REASON_SHUTDOWN,
                PARTIAL_REASON_CLOCK_CHANGED,
            ]
        );
        // 去重：同一个语义写两个字符串是下游最难查的那类 bug。
        let unique: BTreeSet<&&str> = PARTIAL_REASONS.iter().collect();
        assert_eq!(unique.len(), PARTIAL_REASONS.len());
    }

    /// 上面的闭集测试两边都是**同一个常量**（改值也照样绿），所以另起一条按**字面量**锁。
    ///
    /// 这几个串是跨语言契约：数据面只搬 `body` 不建模，平台按字面量解释 ——
    /// 在 Rust 侧改个值不会有任何编译错误，但平台会把 `in`/`out` 统计成未知方向。
    #[test]
    fn wire_vocabulary_literals_are_stable() {
        assert_eq!(RECORD_SCHEMA_V1, "wist.host_flow/v1");
        assert_eq!(SOURCE_HOST_EBPF, "host_ebpf");
        assert_eq!(COVERAGE_HOST_PARTIAL, "host_partial");

        assert_eq!(DIRECTION_IN, "in");
        assert_eq!(DIRECTION_OUT, "out");
        assert_eq!(DIRECTIONS, ["in", "out"].as_slice());

        assert_eq!(CAPTURE_TC_INGRESS, "tc_ingress");
        assert_eq!(CAPTURE_TC_EGRESS, "tc_egress");
        assert_eq!(CAPTURES, ["tc_ingress", "tc_egress"].as_slice());

        assert_eq!(WINDOW_SECONDS_DEFAULT, 60);

        assert_eq!(PARTIAL_REASON_AGENT_START, "agent_start");
        assert_eq!(PARTIAL_REASON_PAUSED, "paused");
        assert_eq!(PARTIAL_REASON_INTERFACE_CHANGED, "interface_changed");
        assert_eq!(PARTIAL_REASON_SHUTDOWN, "shutdown");
        assert_eq!(PARTIAL_REASON_CLOCK_CHANGED, "clock_changed");
        assert_eq!(
            PARTIAL_REASONS,
            [
                "agent_start",
                "paused",
                "interface_changed",
                "shutdown",
                "clock_changed",
            ]
            .as_slice()
        );

        assert_eq!(WIRE_BYTES_SOURCE_WIRE_LEN, "wire_len");
        assert_eq!(WIRE_BYTES_SOURCE_UNAVAILABLE, "unavailable");
        assert_eq!(WIRE_BYTES_SOURCES, ["wire_len", "unavailable"].as_slice());
    }

    /// 全部可选字段都给值的记录（ICMP + `wire_bytes` + 截断窗口）。
    fn full_record() -> HostFlowRecord {
        HostFlowRecord {
            icmp_type: Some("8".to_string()),
            icmp_code: Some("0".to_string()),
            port_unavailable: Some(true),
            wire_bytes: Some(1514),
            wire_bytes_source: WIRE_BYTES_SOURCE_WIRE_LEN.to_string(),
            partial: Some(true),
            partial_reason: Some(PARTIAL_REASON_AGENT_START.to_string()),
            ..record()
        }
    }

    /// 键集的**另一半分**：上一条只覆盖了「可选字段全部省略」的形态，
    /// 新增一个可选字段在那种形态下根本看不见（它是 `None`，不出现）。
    /// 两条合起来才能锁住 34 个键的完整契约。
    #[test]
    fn the_full_wire_field_set_is_exactly_the_documented_one() {
        let value = serde_json::to_value(full_record()).expect("serialize");
        let keys: BTreeSet<&str> = value
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        let expected: BTreeSet<&str> = [
            "record_schema",
            "source",
            "coverage",
            "capture",
            "collector_instance_id",
            "host_id",
            "host_name",
            "kernel",
            "ifname",
            "ifindex",
            "if_mac",
            "local_addrs",
            "direction",
            "protocol",
            "srcaddr",
            "dstaddr",
            "srcport",
            "dstport",
            "icmp_type",
            "icmp_code",
            "port_unavailable",
            "start",
            "end",
            "window_seconds",
            "packets",
            "bytes",
            "wire_bytes",
            "wire_bytes_source",
            "skb_count",
            "gso_skb_count",
            "truncated",
            "dropped",
            "partial",
            "partial_reason",
        ]
        .into_iter()
        .collect();
        assert_eq!(keys, expected);
        assert_eq!(keys.len(), 34);

        // 可选字段都有值时也要能原样读回来（`skip_serializing_if` 不能把值吃掉）。
        let text = serde_json::to_string(&full_record()).expect("serialize");
        let parsed: HostFlowRecord = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(parsed, full_record());
    }

    /// `deny_unknown_fields` 只管**多键**；少键得另有一条 ——
    /// 非可选字段是「数据面/平台可以无条件读」的前提。
    #[test]
    fn required_fields_cannot_be_missing() {
        let value = serde_json::to_value(record()).expect("serialize");
        for required in [
            "record_schema",
            "collector_instance_id",
            "host_id",
            "kernel",
            "ifname",
            "ifindex",
            "local_addrs",
            "direction",
            "protocol",
            "srcaddr",
            "dstaddr",
            "srcport",
            "dstport",
            "start",
            "end",
            "window_seconds",
            "packets",
            "bytes",
            "wire_bytes_source",
            "truncated",
            "dropped",
        ] {
            let mut broken = value.clone();
            broken.as_object_mut().expect("object").remove(required);
            let err = serde_json::from_value::<HostFlowRecord>(broken)
                .expect_err(&format!("缺 {required} 必须拒收"));
            assert!(err.to_string().contains(required), "{required}: {err}");
        }
    }

    /// 设计 §6 的示例 JSON 与数据面的 fixture（`models/wpl/agent_uplink/sample.dat`）
    /// 是同一条 —— 它就是「线上到底长什么样」。这里把它锁住：
    /// 类型（`bytes` 是数字、`protocol`/`srcport` 是字符串）、可选字段的省略、取值。
    #[test]
    fn the_documented_sample_json_round_trips_unchanged() {
        const SAMPLE: &str = r#"{"record_schema":"wist.host_flow/v1","source":"host_ebpf","coverage":"host_partial","capture":"tc_egress","collector_instance_id":"agent-host-d97e23da6111","host_id":"host-d97e23da6111","host_name":"vm-web-01","kernel":"5.15.0-91-generic","ifname":"eth0","ifindex":2,"if_mac":"52:54:00:aa:bb:cc","local_addrs":["10.118.0.14"],"direction":"out","protocol":"6","srcaddr":"10.118.0.14","dstaddr":"88.214.25.121","srcport":"50123","dstport":"443","start":1789469760,"end":1789469820,"window_seconds":60,"packets":13,"bytes":1460,"wire_bytes_source":"unavailable","skb_count":4,"gso_skb_count":1,"truncated":false,"dropped":0}"#;
        let expected: serde_json::Value = serde_json::from_str(SAMPLE).expect("sample is json");
        let parsed: HostFlowRecord =
            serde_json::from_value(expected.clone()).expect("sample 应该能读成 v1 记录");
        assert_eq!(serde_json::to_value(&parsed).expect("serialize"), expected);

        // 抽查口径（类型错了这条会红）。
        assert_eq!(parsed.protocol, "6");
        assert_eq!(parsed.srcport, "50123");
        assert_eq!(parsed.bytes, 1460);
        assert_eq!(parsed.window_seconds, 60);
        assert_eq!(parsed.direction, DIRECTION_OUT);
        assert_eq!(parsed.capture, CAPTURE_TC_EGRESS);
        assert_eq!(parsed.wire_bytes, None);
    }
}
