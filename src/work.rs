//! **工作授权快照**：网关授权（grant）、agentd 拉取（`WorkGrant`）并确认。
//!
//! ## 为什么是「快照」而不是「指令流」
//!
//! 常驻工作（`StandingWork`）表达的是**期望状态**：某个采集面应当持续执行什么。
//! 期望状态天然是幂等的（同一份重复拉取不产生副作用），断了网、重启了进程，
//! 再拉一次就回到期望 —— 不必让网关记住「上次推到哪条」。
//! 这跟 `PollControlCommands` 的长轮询指令流是两种东西：那条要求**不重放**，这条要求**可重拉**。
//!
//! 一次性工作（`OneShotWork`）是命令式的，但它也搭这份快照回来：
//! 决定「现在还该做吗」的是它的状态（未了结才出现在快照里），不是「推没推过」。
//!
//! ## 为什么在契约 crate
//!
//! 这是**两侧都要解析**的字节：网关写授权、agentd 读并执行。各写一份结构体，
//! 迟早出现「网关发了 `plan_version`、agentd 读的是 `version`」这种只能在真机联调时才发现的分叉。
//! 与 `DiscoveryAspectPolicySet` 同一个理由。
//!
//! ## 与本地保护暂停的区别
//!
//! agentd 自己也会暂停（如 `spool over limit`），那是**本地保护**：自动、临时、本机可见。
//! 这里下发的 `paused` 是**授权层状态**：人工、持久、跨重启。两者在观测里必须能分辨，
//! 不要合并成一个 paused —— 合并之后「谁把采集停了」就再也说不清。

use serde::{Deserialize, Serialize};

use crate::capability_report::LogsCapabilities;

/// 常驻工作的状态取值（对应模型 `StandingWork.status`）。
///
/// `superseded` 与 `revoked` 都不出现在 `WorkGrant.standing` 里：前者是「被新版本取代」，
/// 后者是「授权被撤」。两者都要留痕，所以还是要有状态而不是删行。
pub const STANDING_WORK_STATUSES: [&str; 4] = ["active", "paused", "superseded", "revoked"];

/// 一次性工作的状态取值（对应模型 `OneShotWork.status`）。
pub const ONE_SHOT_WORK_STATUSES: [&str; 9] = [
    "dispatched",
    "accepted",
    "running",
    "paused",
    "succeeded",
    "failed",
    "timed_out",
    "canceled",
    "expired",
];

/// Agent **允许上报**的一次性工作状态（[`ONE_SHOT_WORK_STATUSES`] 的真子集）。
///
/// 为什么不是全集：`dispatched` 是网关自己写的（派下去那一刻），`paused` / `canceled` / `expired`
/// 归**控制面与期限**管（运维暂停撤回、或过了截止）—— agent 无权把它们写回去。
/// 剩下的问题只有一种：「这件活做完了没有」，答案就这三种。
pub const AGENT_REPORTABLE_WORK_STATUSES: [&str; 3] = ["running", "succeeded", "failed"];

/// 一次性工作的**终态**：到了这几个状态就了结了，不再出现在快照里。
pub const ONE_SHOT_TERMINAL_STATUSES: [&str; 5] =
    ["succeeded", "failed", "timed_out", "canceled", "expired"];

/// 工作类型：常驻（持续到被替换或撤回）与一次性（有期限与终态）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "state", domain = "Control", module = "Control.Agent.Work")]
#[serde(rename_all = "PascalCase")]
pub enum WorkKind {
    /// 常驻工作：按**采集面**授权，一个面一份。
    Standing,
    /// 一次性工作：按**动作**授权。
    OneShot,
}

/// 常驻工作：网关声明该 Agent 应当持续执行的工作。
///
/// 粒度是**采集面**（一个面 = 一份工作），不是 capability：一份 `collect_logs` 会裹住十几个面，
/// 那样「按面暂停 / 限流 / 审计」全都无从下手。面由模板的 `family_scope` 展开而来，
/// **不携带 capability** —— 该面由哪个采集器承接，由 `spec` 里的单元各自决定。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Control", module = "Control.Agent.Work")]
#[serde(deny_unknown_fields)]
pub struct StandingWork {
    pub work_id: String,
    pub agent_id: String,
    /// 采集面（`CollectionFamily`）。
    pub family: String,
    /// 工作参数：由采集目录的条目组合而成（**不是自由文本**），随 `plan_version` 整体替换。
    pub spec: String,
    /// 本工作按哪一版目录展开：目录换版**不追改**已授权工作（要跟新版得走新提案 + 审定）。
    pub catalog_version: i64,
    /// 生效依据：指向已批准的提案（人工直填 spec 时为空）。
    #[serde(default)]
    pub proposal_id: Option<String>,
    /// 期望版本：网关每次改动 +1；agentd 回报实际版本，与它比对即得漂移。
    pub plan_version: i64,
    pub effective_from: String,
    /// 见 [`STANDING_WORK_STATUSES`]。
    pub status: String,
    pub updated_by: String,
    pub updated_at: String,
}

/// 一次性工作：有计划开始时间与期限，有明确终态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Control", module = "Control.Agent.Work")]
#[serde(deny_unknown_fields)]
pub struct OneShotWork {
    pub work_id: String,
    pub agent_id: String,
    /// 动作面：upgrade / snapshot / exec / ...（执行载体是 Reporting 域的 ActionPlan）。
    pub action: String,
    pub spec: String,
    /// 计划开始时间：到点前不应执行（与「立即派发」区分开）。
    pub scheduled_at: String,
    /// 绝对截止：业务要求的时间点，**暂停也照走**（不由暂停顺延）。
    pub deadline_at: String,
    /// 执行预算（秒）：**只在实际执行时消耗**，暂停期间不计。
    pub timeout_seconds: i64,
    /// 可中断性：只有可中断的动作才允许运行中暂停。
    pub interruptible: bool,
    /// 见 [`ONE_SHOT_WORK_STATUSES`]。
    pub status: String,
    /// 当前暂停的起点（运行中暂停才有；恢复后清空）。
    #[serde(default)]
    pub paused_at: Option<String>,
    /// 累计暂停时长（秒）：用于审计与「预算未被暂停消耗」的核对。
    pub paused_total_seconds: i64,
    /// 步级断点：已完成步骤保留、`current_step` 在恢复时重做。
    #[serde(default)]
    pub current_step: Option<String>,
    #[serde(default)]
    pub completed_steps: Vec<String>,
    /// 已尝试次数（含恢复后的重做）。
    pub attempt: i64,
    pub issued_by: String,
    pub issued_at: String,
}

impl OneShotWork {
    /// 是否**未了结**（快照里只带未了结的活）。
    pub fn is_outstanding(&self) -> bool {
        !ONE_SHOT_TERMINAL_STATUSES.contains(&self.status.as_str())
    }

    /// 是否允许在运行期间暂停。
    ///
    /// 两条都要满足：动作声明了 `interruptible`，且此刻确实在做（`accepted`/`running`）。
    /// 不可中断的动作**拒绝**而不是「尽力暂停」—— 挂起半个升级进程比不暂停更危险。
    pub fn can_pause(&self) -> bool {
        self.interruptible && matches!(self.status.as_str(), "accepted" | "running")
    }
}

/// 管理面授权或撤回工作的回执：一份工作一次。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ::jumo_derive::Jumo)]
#[jumo(kind = "struct", domain = "Control", module = "Control.Agent.Work")]
#[serde(deny_unknown_fields)]
pub struct WorkReceipt {
    pub work_id: String,
    pub agent_id: String,
    pub work_kind: WorkKind,
    /// accepted | paused | resumed | revoked | rejected。
    pub status: String,
    pub plan_version: i64,
    pub created_at: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// 工作参数（`StandingWork.spec` 的内容）
// ─────────────────────────────────────────────────────────────────────────────
//
// 模型里 `StandingWork.spec` 是一个 `String`，语义是「工作参数：由采集目录的条目
// 组合而成，**不是自由文本**」。这里的三个类型就是它的**编码**：一串已物化的采集单元。
//
// 为什么必须带上来源与规则标识，而不是只给一串 `unit_id`：agentd 拿到工作要能
// **直接照做**。单元的采集来源（`sources`）与数据面规则标识（`rule_ref`）本来都只
// 存在于网关的采集目录里，只发 id 等于发了一张自己去不了的地址 —— 于是要么再去网关
// 拉一次目录（多一条必须鉴权的路径），要么两边各维护一份目录（必然漂移）。
//
// 为什么不把它们塞成 `StandingWork` 的字段：那会把「工作」与「内容目录」的边界糊掉；
// 模型里这个字段就是**不透明的工作参数**，保持它不透明是两侧能独立演进的前提。

/// 一份常驻工作的工作参数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkSpec {
    /// 本工作包含的采集单元（已按该机器的事实裁剪过）。
    #[serde(default)]
    pub units: Vec<WorkSpecUnit>,
}

/// 一个已物化的采集单元：只说「采什么、怎么落地」，不复述策展元信息。
///
/// 刻意不带 `status` / `match` / `catalog_version`：那些是网关策展与裁剪的输入，
/// 工作一旦发出去就已经裁剪完了。带上它们会让 agentd 有「再判断一次」的空间，
/// 而 agentd 不是第二个策展器。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkSpecUnit {
    pub unit_id: String,
    /// `collect_logs` | `collect_metrics`：该单元由哪类采集器承接。
    pub capability: String,
    /// 数据面 rule/oml 标识（空 = 该单元还没接上规则，理论不会入工作）。
    #[serde(default)]
    pub rule_ref: String,
    /// `none` | `root` | `fda`：要采到这东西得有什么权限 —— 缺权限时应**说清缺什么**，
    /// 而不是安静地采不到。
    #[serde(default)]
    pub requires_privilege: String,
    #[serde(default)]
    pub sources: Vec<WorkSpecSource>,
}

/// 采集来源：`kind` ∈ `FileGlob` | `Exporter` | `UnifiedLogPredicate` | `MetricInterval`。
///
/// 与模型 `CollectionSource` 同形：`target` 的含义由 `kind` 决定
/// （路径通配 / 导出器标识 / 谓词 / 周期）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkSpecSource {
    pub kind: String,
    pub target: String,
    /// 怎么读这条来源：`none`（一行一条）| `indented`（行首缩进是上一条的续行）|
    /// `mysql_slow`（`# Time:` 起块）| `postgres_csv`（**引号外**的换行才切记录）。
    ///
    /// 只对可 tail 的 `FileGlob` 有意义，其余 kind 恒为 `none`。**不是**"要不要多行"这个偏好，
    /// 而是这条日志的**固有格式**：说错了要么把多行记录拆散、要么把独立记录粘成一条。
    #[serde(default = "default_multiline")]
    pub multiline: String,
    /// 从 `target` 匹配集合里剔掉的**绝对路径 glob**（默认空）。
    ///
    /// 只对 `FileGlob` 有意义，按**展开后的完整路径**匹配（不是文件名）。为什么需要它：
    /// 容器日志的选源根是 `/var/log/containers/*.log`，而目录里同时有不该采的容器
    /// （sidecar / init / 平台自身的 pod）；没有排除项就只能把业务服务名写进 pattern，
    /// 而服务名会随集群漂移。
    ///
    /// 空数组**不序列化**：旧 agent 的 `WorkSpecSource` 是 `deny_unknown_fields` 的，
    /// 发一个空字段过去只会给旧机队添一种没必要的解析失败。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude_globs: Vec<String>,
    /// 容器选源的结构化过滤器（默认空）。只对 `FileGlob` 有意义。
    ///
    /// 空（两个列表都空）**不序列化**：与 `exclude_globs` 同一个理由 —— 旧 agent 的
    /// `WorkSpecSource` 是 `deny_unknown_fields` 的，发一个空字段过去只会给旧机队
    /// 添一种没必要的解析失败。
    ///
    /// ⚠ 非空时**要求 agent 支持**（与 glob 单元同一条纪律）：能力轴是
    /// `LogsCapabilities.filters`（`namespace_in` / `pod_not_in`），网关在派活时裁掉
    /// 不支持的来源；旧 agent 收到非空 `filters` 会拒收整份 spec。
    #[serde(default, skip_serializing_if = "ContainerFilters::is_empty")]
    pub filters: ContainerFilters,
    /// 读进来的记录里，含这些**子串**的整条丢掉（默认空 = 不丢）。只对 `FileGlob` 有意义。
    ///
    /// 为什么是子串、不是正则：这是**噪音行**的白名单（`pam_unix(cron:session)` 这类
    /// 每分钟一条、信息量为零的行），不是解析。正则意味着在采集端维护一套可执行表达式
    /// （回溯、超时、误匹配都是新故障面），而子串能做到的事不需要它。
    ///
    /// 语义是“整条记录”：子串命中就丢整条，**不会只删命中那一行**而留下半条多行记录。
    /// 丢弃数进 agentd 自观测（`telemetry noise dropped …`），不然“怎么少了”无处可查。
    ///
    /// 空数组**不序列化**：旧 agent 的 `WorkSpecSource` 是 `deny_unknown_fields` 的。
    /// 非空时要求 agent 支持 `LogsCapabilities.drop_if_contains`（`substring`），网关据此裁源。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drop_if_contains: Vec<String>,
}

/// 容器日志的选源根（写死，D4）。
///
/// `/var/log/pods` 下有 `0.log` / `1.log` 与重启序号歧义；`/var/log/containers` 里
/// 每个活容器恰好一条 symlink，文件名自带 pod / ns / container。
pub const CONTAINER_LOG_ROOT: &str = "/var/log/containers";

/// 容器选源的结构化过滤器（默认全空 = 不过滤）。
///
/// 为什么不是「再写几条 pattern」：容器日志文件名本身就是
/// `<pod>_<namespace>_<container>-<id>.log`，选源要用的三个维度全在里面；把 namespace
/// 白名单散成 N 条 path pattern 会随集群漂移，而过滤器是**结构化**的、与文件名格式解耦。
///
/// 求值位置：**发现阶段**（agentd 展开 glob、剔完 `exclude_globs` 之后，读之前），
/// 读取器不参与 —— 所以它不影响 tail / checkpoint / 轮转语义。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerFilters {
    /// 只采这些 namespace（空 = 不限）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub namespace_in: Vec<String>,
    /// 这些 pod 不采（空 = 不排除）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pod_not_in: Vec<String>,
}

/// 容器过滤器的维度名（**能力词表**的值）：与 [`ContainerFilters`] 的字段同名。
///
/// 为什么能力名 = 字段名：网关的报错文案（`filters:namespace_in`）与目录里的写法
/// （`filters = { namespace_in = [...] }`）能一一对上，排查时不用再查一份映射表。
pub const FILTER_NAMESPACE_IN: &str = "namespace_in";
/// 见 [`FILTER_NAMESPACE_IN`]。
pub const FILTER_POD_NOT_IN: &str = "pod_not_in";

/// agentd 能承接的**全部**容器过滤维度（与 [`ContainerFilters::used_dimensions`] 同源）。
///
/// 新增一个过滤维度时：先在 [`ContainerFilters`] 里加字段，再往这里加名字 ——
/// 两侧对不上的漂移由 `a_fully_declared_agent_is_missing_nothing_the_contract_calls_executable` 钉住。
pub const CONTAINER_FILTERS: &[&str] = &[FILTER_NAMESPACE_IN, FILTER_POD_NOT_IN];

/// 噪音行丢弃的**求值方式**（能力词表的值）。
///
/// 目前只有子串一种 —— 它跟正则不是同一个东西（见 [`WorkSpecSource::drop_if_contains`]）。
/// 以后真要加正则，那是新的一项能力：旧 agent 不该被当成“也懂正则”。
pub const DROP_IF_CONTAINS_SUBSTRING: &str = "substring";
/// agentd 能承接的全部噪音行丢弃方式（目前只有子串）。
pub const DROP_IF_CONTAINS_MODES: &[&str] = &[DROP_IF_CONTAINS_SUBSTRING];

/// `drop_if_contains` 的条数上限（超过就是“在采集端写解析规则”，那条路不该走）。
pub const DROP_IF_CONTAINS_MAX_ITEMS: usize = 8;
/// 单个子串的长度上限（字节）。
pub const DROP_IF_CONTAINS_MAX_BYTES: usize = 200;

/// 校验 `drop_if_contains` 的形状：精确子串，不是 pattern。
///
/// 网关装载期、agentd 配置校验、web 表单三处共用同一口径（否则会出现“页面能填、
/// agentd 不认”）。只拦“一定有意义不明”的形态：空串 / 首尾空白 / 控制字符 / 过长 / 太多 / 重复。
pub fn validate_drop_if_contains(needles: &[String]) -> Result<(), String> {
    if needles.len() > DROP_IF_CONTAINS_MAX_ITEMS {
        return Err(format!(
            "drop_if_contains 条数过多（{}，上限 {DROP_IF_CONTAINS_MAX_ITEMS}）",
            needles.len()
        ));
    }
    for (index, needle) in needles.iter().enumerate() {
        if needle.trim().is_empty() {
            return Err(format!("drop_if_contains[{index}] 是空串"));
        }
        if needle.trim() != needle {
            return Err(format!("drop_if_contains[{index}] {needle:?} 带首尾空白"));
        }
        if needle.chars().any(char::is_control) {
            return Err(format!("drop_if_contains[{index}] {needle:?} 含控制字符"));
        }
        if needle.len() > DROP_IF_CONTAINS_MAX_BYTES {
            return Err(format!(
                "drop_if_contains[{index}] 过长（{} 字节，上限 {DROP_IF_CONTAINS_MAX_BYTES}）",
                needle.len()
            ));
        }
        if needles[..index].contains(needle) {
            return Err(format!("drop_if_contains[{index}] {needle:?} 重复"));
        }
    }
    Ok(())
}

impl ContainerFilters {
    /// 两个列表都空 = 不过滤（空过滤器不上线，见 [`WorkSpecSource::filters`]）。
    pub fn is_empty(&self) -> bool {
        self.namespace_in.is_empty() && self.pod_not_in.is_empty()
    }

    /// 这条来源**实际用到**的过滤器维度（空 = 不过滤）。
    ///
    /// 能力判定（[`missing_log_capabilities`]）与 agentd 的能力上报都从这里取：
    /// 只写一遍字段 → 能力名的对应关系，少一处「新增一个过滤字段、忘了加进能力词表」的漏判。
    pub fn used_dimensions(&self) -> Vec<&'static str> {
        let mut used = Vec::new();
        if !self.namespace_in.is_empty() {
            used.push(FILTER_NAMESPACE_IN);
        }
        if !self.pod_not_in.is_empty() {
            used.push(FILTER_POD_NOT_IN);
        }
        used
    }

    /// 形状校验：过滤器里的值是**精确名字**，不是 pattern。
    ///
    /// 网关、web、agentd 三处共用这一份（否则会出现「页面能填、agentd 不认」）。
    /// 不强制 RFC1123：这里只拦“一定对不上”的形态（空串 / 空白 / 通配符 / 超长），
    /// 名字是否真存在由集群决定，不是配置校验能回答的。
    pub fn validate(&self) -> Result<(), String> {
        for (field, values) in [
            ("namespace_in", &self.namespace_in),
            ("pod_not_in", &self.pod_not_in),
        ] {
            for value in values {
                if value.trim().is_empty() {
                    return Err(format!("{field} 里有空值"));
                }
                if value.trim() != value {
                    return Err(format!("{field} 里 {value:?} 带首尾空白"));
                }
                if value.contains(['*', '?', '[', ']']) {
                    return Err(format!(
                        "{field} 里 {value:?} 含通配符（这里是精确名字，不是 pattern）"
                    ));
                }
                if value.len() > 253 {
                    return Err(format!("{field} 里 {value:?} 过长（>253）"));
                }
            }
        }
        Ok(())
    }
}

fn default_multiline() -> String {
    "none".to_string()
}

impl WorkSpec {
    /// 解析 `StandingWork.spec`。
    ///
    /// 解析失败**不当作空工作**：空工作会让「参数坏了」退化成「没事可做」，
    /// 两种情形的运维动作完全不同。
    pub fn parse(spec: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(spec)
    }

    /// 序列化（网关写 `spec` 用）。字段顺序固定 → 同一份工作每次编码都一样，
    /// `plan_version` 之外的字节也稳定，方便对账。
    pub fn encode(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// 周期类来源声明的间隔（秒）：取所有 `MetricInterval` 里最密的那一个。
    ///
    /// 取最密而不是取第一个：一份工作里若有两个指标单元，采慢的那个会
    /// 静默压掉采密的要求，而“要得最急”的需求才是上送频率的下界。
    pub fn metric_interval_seconds(&self) -> Option<i64> {
        self.units
            .iter()
            .flat_map(|unit| unit.sources.iter())
            .filter(|source| source.kind == "MetricInterval")
            .filter_map(|source| parse_interval_seconds(&source.target))
            .min()
    }

    /// 是否要采日志（有 `collect_logs` 单元）。
    pub fn collects_logs(&self) -> bool {
        self.units
            .iter()
            .any(|unit| unit.capability == "collect_logs")
    }
}

/// agentd **有可能真的执行**的来源 kind（其余 kind 一律 `unsupported`，不当没看见）。
///
/// 注意：kind 对了**还不够** —— 目标形态也有限制，见 [`is_executable_source`]：
/// `FileGlob` 要是**绝对路径**（显式路径或受支持的 glob）、`Exporter` 要是**已知导出器 ID**。
/// 只按 kind 判会把「`**` 递归」「`~` 与相对路径」「写错的导出器」当成可采，而那在采集端落不了地。
pub const EXECUTABLE_SOURCE_KINDS: &[&str] = &["FileGlob", "MetricInterval", "Exporter"];

/// 目标里有没有通配元字符 —— 有就说明它不是**显式单路径**。
pub fn has_glob_meta(target: &str) -> bool {
    target.contains(['*', '?', '['])
}

/// 这个目标是不是采集端今天**真能打开**的路径：**绝对路径、无通配**。
///
/// 采集端（agentd 第一版）只实现显式单路径输入：**不做 glob 展开、也不展开 `~`**
/// （见 `wist/wist-agentd/docs/design/log-file-input-spec.md` §2 的能力边界）。
/// 单层 glob 从 W1 起可采，但那走 [`is_glob_path`]；`~` 与相对路径仍永远不可采。
///
/// 从 W1 起，带通配的目标由 [`is_glob_path`] / [`is_file_target`] 判定 —— 它们才是
/// 「采集端接不接得了」的口径；本函数只留给**单文件**场景（例如「一个显式路径能否参与
/// 存在性裁剪」：那一步只能对显式路径做，探测不到的路径不能当不存在）。
pub fn is_explicit_path(target: &str) -> bool {
    target.starts_with('/') && !has_glob_meta(target)
}

/// 这个目标是不是**受支持的文件族 glob**：**绝对路径、有通配、不含 `**`**。
///
/// `**` 被拒不是保守，而是口径：采集端只扫 pattern 的固定前缀目录、`*` 不跨目录
/// （`literal_separator`），递归展开既没实现也无法给出上限 —— 让它在目录装载期就被挡住，
/// 比让 agent 每轮报一次 IO 错误好。
pub fn is_glob_path(target: &str) -> bool {
    target.starts_with('/') && has_glob_meta(target) && !target.contains("**")
}

/// 这个目标是不是采集端**能承接的文件目标**：显式绝对路径，或受支持的文件族 glob。
///
/// 不展开 `~`、不接受相对路径：相对路径的结果取决于进程 cwd，同一台机器换一种启动方式
/// 就换一个采集对象，那不是「配置」，是运气。
pub fn is_file_target(target: &str) -> bool {
    is_explicit_path(target) || is_glob_path(target)
}

/// 内核日志导出器（W3）：`journalctl -k -p 4 -o json`，**靠游标续读**。
///
/// 与其余导出器不同，它是**有状态**的：上一轮读到的游标落盘，下一轮从那里接着读。
/// 契约只承诺 ID 与「可执行」；游标怎么存、失效（被 vacuum）怎么回退是采集端的事
/// （见 `linux-server-log-coverage-plan.md` §6 W3 / D6）。
///
/// 为什么单独给个常量：agentd 的 `argv` 与状态处理要**按 ID 分支**，字面量散在两侧
/// 迟早会漂（一处改名、另一处还在按旧名特判，表现就是「游标不生效、每轮重读」）。
pub const KERNEL_LOG_EXPORTER: &str = "journalctl-kernel";

/// 这条来源缺哪些日志能力（空 = 齐了）。
///
/// `None` = 这个 kind 不归 logs 能力管（`MetricInterval` 是指标；未知/不可执行的形态
/// 已经被 [`is_executable_source`] 拦下，不需要第二套说法）。
///
/// 返回值是「桶:值」（`file_inputs:glob` / `multiline_modes:indented` /
/// `filters:namespace_in` / `drop_if_contains:substring` / `exporters:journalctl-kernel`），
/// 可以直接用在拒绝文案与自观测里。
///
/// 为什么收整条来源而不是拆成 `kind` / `target` / `multiline`：每加一个**选源/读法字段**
/// （W2 的 `filters`、W4 的 `drop_if_contains`）都要重新讨论「要不要传」，而“忘了传”
/// 在这里的后果是**整份 spec 被旧 agentd 拒收**（`deny_unknown_fields`），不是少采一条。
/// 收整条来源，新增字段时编译器会直接把所有调用点指出来。
///
/// 为什么把判定放契约：网关（派活时）与 agentd（上报时）必须用**同一份**词表。
/// 两边各写一份的结果是「agent 说自己支持 glob、网关按一套旧名单判它不支持」——
/// 而且这种矛盾只能靠人去两台机器上对日志才能发现。
pub fn missing_log_capabilities(
    source: &WorkSpecSource,
    declared: &LogsCapabilities,
) -> Option<Vec<String>> {
    let declared_has = |bucket: &[String], value: &str| bucket.iter().any(|item| item == value);
    let kind = source.kind.as_str();
    let target = source.target.as_str();

    match kind {
        "FileGlob" => {
            // 不可执行的形态交给 `is_executable_source` 说，不在这里扣一顶“能力不够”的帽子。
            if !is_file_target(target) {
                return None;
            }
            let mut missing = Vec::new();
            let need = if is_glob_path(target) {
                FILE_INPUT_GLOB
            } else {
                FILE_INPUT_ABSOLUTE_PATH
            };
            if !declared_has(&declared.file_inputs, need) {
                missing.push(format!("file_inputs:{need}"));
            }
            // `none` 是老读法（一行一条），不算一项需要协商的能力。
            if source.multiline != "none"
                && !declared_has(&declared.multiline_modes, &source.multiline)
            {
                missing.push(format!("multiline_modes:{}", source.multiline));
            }
            // 过滤器是**选源层**的能力（与读法正交）：旧 agentd 收到 `filters` 会拒收整份
            // spec，所以逐维度点名。
            for dimension in source.filters.used_dimensions() {
                if !declared_has(&declared.filters, dimension) {
                    missing.push(format!("filters:{dimension}"));
                }
            }
            // 噪音行丢弃（W4）：同样是新字段，旧 agentd 会把整份 spec 当未知字段拒收。
            // 注意：网关对这项**不裁源**，而是去掉字段降级（它是减法，少采不如多采）；
            // 这里仍如实报告“缺这项能力”，供其它消费方与诊断使用。
            if !source.drop_if_contains.is_empty()
                && !declared_has(&declared.drop_if_contains, DROP_IF_CONTAINS_SUBSTRING)
            {
                missing.push(format!("drop_if_contains:{DROP_IF_CONTAINS_SUBSTRING}"));
            }
            Some(missing)
        }
        "Exporter" => {
            let (id, _) = parse_exporter_target(target)?;
            Some(if declared_has(&declared.exporters, id) {
                Vec::new()
            } else {
                vec![format!("exporters:{id}")]
            })
        }
        _ => None,
    }
}

/// agentd 能承接的**文件输入形态**（能力名）。
///
/// 与 [`is_file_target`] 同源：判定能采的形态，就是这里要声明的形态 —— 两边各写一份必然漂。
pub const FILE_INPUT_ABSOLUTE_PATH: &str = "absolute-path";
/// 见 [`FILE_INPUT_ABSOLUTE_PATH`]。
pub const FILE_INPUT_GLOB: &str = "glob";

/// agentd 支持的 `multiline` 模式（与 [`WorkSpecSource::multiline`] 的取值同源）。
///
/// * `none` / `indented`：W1；
/// * `mysql_slow`：MySQL slow log，`# Time:` 起块（W4）；
/// * `postgres_csv`：PostgreSQL csvlog，**引号外**的换行才切记录（W4）。
pub const MULTILINE_MODES: &[&str] = &["none", "indented", "mysql_slow", "postgres_csv"];

/// 定时导出器（`Exporter`）的**已知 ID**：协议层词表，网关 / agentd / web 三侧共用。
///
/// 为什么词表放这里：`is_executable_source` 要判「这条 `Exporter` 目标今天真能不能采」，
/// 而网关的采集就绪度与 agentd 的「接不接得了」必须是**同一个判据**（见 [`is_executable_source`]）。
/// 词表只收**已实现**的导出器 —— 收录一个 ID 就等于承诺 agentd 能跑它；未知 ID（旧网关发新 ID、
/// 或写错）一律当不可采，不要「写了就算」。新增能力 = 四侧（本 crate + 网关 + agentd + web）
/// 一起改 + 知识库把对应单元开 `active`。
pub const EXPORTER_IDS: &[&str] = &[
    "journalctl-unit",
    "journalctl-shutdown",
    "last-reboot",
    "nft-ruleset",
    "iptables-save",
    "smartctl",
    "dmesg",
    "auditd-execve",
    KERNEL_LOG_EXPORTER,
];

/// 把一个 `Exporter` 目标拆成 `(id, arg)`：`"dmesg:panic"` → `("dmesg", Some("panic"))`。
///
/// 无 `:` 时 `arg` 为 `None`；空 id 或含空白的 id 视为无效（返回 `None`）。`arg` 是**声明式的窄选项**
/// （如 `dmesg` 的过滤键），不是命令行片段 —— 实现侧各自解释，绝不拼进 shell。
pub fn parse_exporter_target(target: &str) -> Option<(&str, Option<&str>)> {
    let (id, arg) = match target.split_once(':') {
        Some((id, arg)) => (id, Some(arg)),
        None => (target, None),
    };
    let id = id.trim();
    if id.is_empty() || id.contains(char::is_whitespace) {
        return None;
    }
    Some((id, arg))
}

/// 这条 `Exporter` 目标的 ID 是不是**已知且已实现**的导出器。
pub fn is_known_exporter(target: &str) -> bool {
    match parse_exporter_target(target) {
        Some((id, _)) => EXPORTER_IDS.contains(&id),
        None => false,
    }
}

/// 一条来源今天**能不能被采**：`kind` 要 agentd 能执行，`target` 还要是它接得住的形态。
///
/// 为什么把两件事放在一个函数里：网关的采集就绪度（「这个面能不能派下去」）与 agentd 的
/// 「这条来源接不接得了」必须是**同一个判据**，否则会出现「网关说可采、agent 拿到后报
/// unsupported」这种没人能发现的矛盾。加一种 kind 支持时**只改这里**。
pub fn is_executable_source(kind: &str, target: &str) -> bool {
    match kind {
        // 本地文件采集：显式绝对路径，或受支持的文件族 glob（无 `**`、无 `~`）。
        "FileGlob" => is_file_target(target),
        // 指标：`target` 是采集周期（如 `15s`、`60s`），不是路径。
        "MetricInterval" => true,
        // 定时导出器：`target` 要是**已知导出器 ID**（可带 `:arg`）。未知 ID 不可执行。
        "Exporter" => is_known_exporter(target),
        _ => false,
    }
}

impl WorkSpecUnit {
    /// 该单元里可以交给本地文件采集器的来源（`FileGlob`）。
    ///
    /// 其它来源（导出器 / 统一日志谓词）不是本地 tail 一个文件能承接的：
    /// 调用方应当把它们当成**还没支持的单元**如实报出来，而不是当没看见。
    pub fn file_sources(&self) -> Vec<&WorkSpecSource> {
        self.sources
            .iter()
            .filter(|source| source.kind == "FileGlob")
            .collect()
    }

    /// 本单元有没有 agentd 目前接不了的来源（kind 不认识，或目标不是可采的绝对路径）。
    pub fn unsupported_sources(&self) -> Vec<&WorkSpecSource> {
        self.sources
            .iter()
            .filter(|source| !is_executable_source(&source.kind, &source.target))
            .collect()
    }

    /// 本单元至少有一条来源是 agentd **今天真能采**的。
    ///
    /// 「能不能采」只看这个：与解析规则（`rule_ref`）无关 —— 采原文不需要规则，
    /// 规则只决定采下来的东西能不能被归类、抽字段。
    pub fn has_executable_source(&self) -> bool {
        self.sources
            .iter()
            .any(|source| is_executable_source(&source.kind, &source.target))
    }
}

/// 解析 `15s` / `60s` / `5m` 这类周期写法（与目录里 `MetricInterval` 的写法一致）。
///
/// 只认秒与分两种后缀：目录里的值是人写的，多一个单位就多一种笔误的可能，
/// 而解析不了时返回 `None` 会让调用方回退到自己的默认值（不静默取 0）。
pub fn parse_interval_seconds(raw: &str) -> Option<i64> {
    let raw = raw.trim();
    if let Some(value) = raw.strip_suffix('s') {
        return value.trim().parse::<i64>().ok().filter(|value| *value > 0);
    }
    if let Some(value) = raw.strip_suffix('m') {
        let minutes = value.trim().parse::<i64>().ok()?;
        // `checked_mul`：`123456789012345678m` 这类输入会让 `minutes * 60` 溢出 i64。
        // 契约是「解析不了返回 None」（调用方回退到自己的默认值），不能让它 panic（debug）
        // 或悄悄回绕成一个错的间隔（release）。
        return minutes.checked_mul(60).filter(|value| *value > 0);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability_report::LogsCapabilities;

    fn one_shot(status: &str, interruptible: bool) -> OneShotWork {
        OneShotWork {
            work_id: "work-1".to_string(),
            agent_id: "agent-1".to_string(),
            action: "upgrade".to_string(),
            spec: "0.1.4".to_string(),
            scheduled_at: "2026-09-23T00:00:00Z".to_string(),
            deadline_at: "2026-09-24T00:00:00Z".to_string(),
            timeout_seconds: 600,
            interruptible,
            status: status.to_string(),
            paused_at: None,
            paused_total_seconds: 0,
            current_step: None,
            completed_steps: vec![],
            attempt: 0,
            issued_by: "admin".to_string(),
            issued_at: "2026-09-23T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn terminal_one_shot_work_is_not_outstanding() {
        // 未了结的活才进快照：终态留在库里供审计，但不必再发给 agent。
        for status in ONE_SHOT_TERMINAL_STATUSES {
            assert!(!one_shot(status, true).is_outstanding(), "{status}");
        }
        for status in ["dispatched", "accepted", "running", "paused"] {
            assert!(one_shot(status, true).is_outstanding(), "{status}");
        }
    }

    #[test]
    fn only_interruptible_running_work_can_pause() {
        // 不可中断：拒绝，而不是「尽力暂停」。
        assert!(!one_shot("running", false).can_pause());
        // 可中断但没在做（还没到点 / 已经了结）：也没什么可暂停的。
        assert!(!one_shot("dispatched", true).can_pause());
        assert!(!one_shot("succeeded", true).can_pause());
        assert!(one_shot("accepted", true).can_pause());
        assert!(one_shot("running", true).can_pause());
    }

    #[test]
    fn work_kind_serializes_as_the_model_names() {
        assert_eq!(
            serde_json::to_string(&WorkKind::Standing).expect("serialize"),
            "\"Standing\""
        );
        assert_eq!(
            serde_json::to_string(&WorkKind::OneShot).expect("serialize"),
            "\"OneShot\""
        );
    }

    #[test]
    fn only_explicit_absolute_paths_are_collectable_file_targets() {
        // `is_explicit_path` 现在只服务「单文件」语义（例如存在性裁剪只能对显式路径做）。
        // 这个判据被两侧共用（网关的采集就绪度 + agentd 的接受逻辑），所以在这里钉死。
        assert!(is_explicit_path("/var/log/install.log"));
        assert!(!is_explicit_path("/var/log/wifi.log*"));
        assert!(!is_explicit_path("/Library/Logs/DiagnosticReports/*.ips"));
        assert!(!is_explicit_path("~/Library/Logs/Homebrew/*"));
        assert!(!is_explicit_path("relative/app.log"));
    }

    #[test]
    fn file_targets_accept_single_level_globs_and_reject_recursion() {
        // W1：`FileGlob` 判据从「显式路径」放宽为「绝对路径，允许通配」。
        assert!(is_glob_path("/var/log/containers/*.log"));
        assert!(is_glob_path("/var/log/pods/*/*/*.log"));
        assert!(is_glob_path("/var/lib/postgresql/*/data/log/*.csv"));
        assert!(is_file_target("/var/log/containers/*.log"));
        // 显式路径仍是合法文件目标（单文件 input 走它）。
        assert!(is_file_target("/var/log/auth.log"));
        // `**` 不展开：采集端只扫固定前缀目录，接受它只会在采集端变成每轮 IO 错误。
        assert!(!is_glob_path("/var/log/**/*.log"));
        assert!(!is_file_target("/var/log/**/*.log"));
        // `~` / 相对路径不展开。
        assert!(!is_glob_path("~/Library/Logs/*.log"));
        assert!(!is_file_target("relative/*.log"));
        // 空目标不是文件目标。
        assert!(!is_file_target(""));
    }

    #[test]
    fn executability_needs_both_a_supported_kind_and_a_supported_target() {
        // 指标：`target` 是周期不是路径，不管显式路径那一条。
        assert!(is_executable_source("MetricInterval", "15s"));
        // 文件：显式路径与单层 glob 都可执行；`**` / `~` 不可执行（网关不能把它们报成“可采”）。
        assert!(is_executable_source("FileGlob", "/var/log/app.log"));
        assert!(is_executable_source(
            "FileGlob",
            "/var/log/containers/*.log"
        ));
        assert!(!is_executable_source("FileGlob", "/var/log/**/*.log"));
        assert!(!is_executable_source("FileGlob", "~/logs/*.log"));
        // 导出器：kind 对了，ID 还得是**已知的**（`last,lastb` 是 macOS 侧、尚未实现）。
        assert!(is_executable_source("Exporter", "smartctl"));
        assert!(is_executable_source("Exporter", "dmesg:panic"));
        assert!(!is_executable_source("Exporter", "last,lastb"));
        // 还没实现的 kind：一律不可执行。
        assert!(!is_executable_source("UnifiedLogPredicate", "syspolicyd"));
    }

    #[test]
    fn exporter_targets_split_into_a_known_id_and_an_optional_arg() {
        assert_eq!(parse_exporter_target("smartctl"), Some(("smartctl", None)));
        assert_eq!(
            parse_exporter_target("dmesg:panic"),
            Some(("dmesg", Some("panic")))
        );
        // 空 id / 含空白的 id 不是合法 ID。
        assert_eq!(parse_exporter_target(":panic"), None);
        assert_eq!(parse_exporter_target(" dm esg"), None);
        // 判定只看冒号前的 ID，`arg` 不参与。
        assert!(is_known_exporter("dmesg:nvidia-xid"));
        assert!(!is_known_exporter("nope"));
    }

    #[test]
    fn the_kernel_log_exporter_is_in_the_contract_vocabulary() {
        // 词表只增不改名：这一条丢了，网关会把内核单元判成「不可采」（`active` 直接被拦），
        // agentd 也会把它报成 unsupported。
        assert!(EXPORTER_IDS.contains(&KERNEL_LOG_EXPORTER));
        assert!(is_known_exporter(KERNEL_LOG_EXPORTER));
        assert!(is_executable_source("Exporter", KERNEL_LOG_EXPORTER));
        // 前缀相似但不是同一个 ID：不能靠 starts_with 之类的模糊匹配混过去。
        assert!(!is_known_exporter("journalctl-kernelx"));
    }

    // ── 能力协商（D7）：词表与判定同源 ──

    /// 一条测试来源：只关心 kind / target / multiline 时用它（不带 filters / exclude）。
    fn source(kind: &str, target: &str, multiline: &str) -> WorkSpecSource {
        WorkSpecSource {
            kind: kind.to_string(),
            target: target.to_string(),
            multiline: multiline.to_string(),
            exclude_globs: Vec::new(),
            filters: ContainerFilters::default(),
            drop_if_contains: Vec::new(),
        }
    }

    /// 一条容器来源（`FileGlob` + 给定过滤器）。
    fn container_source(target: &str, filters: ContainerFilters) -> WorkSpecSource {
        WorkSpecSource {
            filters,
            ..source("FileGlob", target, "none")
        }
    }

    /// [`missing_log_capabilities`] 的简写：不带筛选条件的来源。
    fn missing(
        kind: &str,
        target: &str,
        multiline: &str,
        declared: &LogsCapabilities,
    ) -> Option<Vec<String>> {
        missing_log_capabilities(&source(kind, target, multiline), declared)
    }

    /// 一个“什么都支持”的 agent 声明（agentd 上报的就是这一份的全集）。
    fn full_logs() -> LogsCapabilities {
        LogsCapabilities {
            file_inputs: vec![
                FILE_INPUT_ABSOLUTE_PATH.to_string(),
                FILE_INPUT_GLOB.to_string(),
            ],
            multiline_modes: MULTILINE_MODES
                .iter()
                .map(|mode| mode.to_string())
                .collect(),
            exporters: EXPORTER_IDS.iter().map(|id| id.to_string()).collect(),
            filters: CONTAINER_FILTERS.iter().map(|di| di.to_string()).collect(),
            drop_if_contains: DROP_IF_CONTAINS_MODES
                .iter()
                .map(|m| m.to_string())
                .collect(),
            ..LogsCapabilities::default()
        }
    }

    #[test]
    fn capability_gaps_name_the_bucket_and_value() {
        let old_agent = LogsCapabilities {
            // W1 之前的 agentd：只会显式路径，不认识导出器能力。
            file_inputs: vec![FILE_INPUT_ABSOLUTE_PATH.to_string()],
            ..LogsCapabilities::default()
        };
        assert_eq!(
            missing("FileGlob", "/var/log/app.log", "none", &old_agent),
            Some(Vec::new())
        );
        assert_eq!(
            missing("FileGlob", "/var/log/app/*.log", "none", &old_agent),
            Some(vec!["file_inputs:glob".to_string()])
        );
        // 多行模式也是能力：说错了会把记录拼碎。
        assert_eq!(
            missing("FileGlob", "/var/log/a.log", "indented", &old_agent),
            Some(vec!["multiline_modes:indented".to_string()])
        );
        // 导出器是具名能力：新 ID（如 W3 的内核日志）旧 agent 一定不认识。
        assert_eq!(
            missing("Exporter", KERNEL_LOG_EXPORTER, "none", &old_agent),
            Some(vec![format!("exporters:{KERNEL_LOG_EXPORTER}")])
        );
        // `arg` 不参与判定（与 `is_known_exporter` 同口径）。
        assert_eq!(
            missing("Exporter", "dmesg:panic", "none", &full_logs()),
            Some(Vec::new())
        );
        // 空的过滤器不要求任何能力：它不上线（`skip_serializing_if`）。
        assert_eq!(
            missing("FileGlob", "/var/log/containers/*.log", "none", &old_agent),
            Some(vec!["file_inputs:glob".to_string()])
        );
    }

    #[test]
    fn container_filter_dimensions_are_negotiated_separately() {
        // 0.9.4 的 agentd：已经有 glob / multiline / exporters，但**不认识 `filters`**。
        // 带 filters 的来源必须被点名 —— 否则那份 spec 会被 `deny_unknown_fields` 整份拒收。
        let before_w2 = LogsCapabilities {
            filters: Vec::new(),
            ..full_logs()
        };
        let namespace_only = ContainerFilters {
            namespace_in: vec!["prod".to_string()],
            pod_not_in: Vec::new(),
        };
        let target = "/var/log/containers/*.log";
        assert_eq!(
            missing_log_capabilities(&container_source(target, namespace_only), &before_w2),
            Some(vec!["filters:namespace_in".to_string()])
        );
        // 两个维度都用到 → 两个都点名（按字段顺序，文案可对账）。
        let both = ContainerFilters {
            namespace_in: vec!["prod".to_string()],
            pod_not_in: vec!["debug-pod".to_string()],
        };
        assert_eq!(
            missing_log_capabilities(&container_source(target, both.clone()), &before_w2),
            Some(vec![
                "filters:namespace_in".to_string(),
                "filters:pod_not_in".to_string(),
            ])
        );
        // 声明了这两个维度就齐了。
        assert_eq!(
            missing_log_capabilities(&container_source(target, both), &full_logs()),
            Some(Vec::new())
        );
        // 能力名就是过滤器的字段名（网关文案与目录写法能一一对上）。
        assert_eq!(
            ContainerFilters {
                namespace_in: vec!["a".to_string()],
                pod_not_in: vec!["b".to_string()],
            }
            .used_dimensions(),
            vec![FILTER_NAMESPACE_IN, FILTER_POD_NOT_IN]
        );
        assert!(ContainerFilters::default().used_dimensions().is_empty());
    }

    #[test]
    fn noise_drop_needles_are_a_capability_of_their_own() {
        // W4 之前的 agentd：有 glob / filters，但**不认识 `drop_if_contains`**。
        // 不点名就会把整份 spec 送给一个 `deny_unknown_fields` 的旧版本。
        let before_w4 = LogsCapabilities {
            drop_if_contains: Vec::new(),
            ..full_logs()
        };
        let mut source = source("FileGlob", "/var/log/auth.log", "none");
        source.drop_if_contains = vec!["pam_unix(cron:session)".to_string()];
        assert_eq!(
            missing_log_capabilities(&source, &before_w4),
            Some(vec!["drop_if_contains:substring".to_string()])
        );
        // 声明了（本版只有子串一种）就齐了。
        assert_eq!(
            missing_log_capabilities(&source, &full_logs()),
            Some(Vec::new())
        );
    }

    #[test]
    fn drop_if_contains_is_substring_shaped_and_stays_out_of_the_wire_when_empty() {
        // 空列表不上线（与 `exclude_globs` / `filters` 同一条纪律）。
        let mut source = source("FileGlob", "/var/log/auth.log", "none");
        assert!(
            !serde_json::to_string(&source)
                .expect("serialize")
                .contains("drop_if_contains")
        );

        source.drop_if_contains = vec!["pam_unix(cron:session)".to_string()];
        let json = serde_json::to_string(&source).expect("serialize");
        assert!(
            json.contains(r#""drop_if_contains":["pam_unix(cron:session)"]"#),
            "{json}"
        );
        assert_eq!(
            serde_json::from_str::<WorkSpecSource>(&json).expect("parse"),
            source
        );

        // 形状校验：空串 / 首尾空白 / 控制字符 / 过长 / 太多 / 重复都要拒。
        assert!(validate_drop_if_contains(&["ok".to_string()]).is_ok());
        assert!(validate_drop_if_contains(&[]).is_ok());
        for bad in ["", " x", "x ", "line\nbreak", " nul\0"] {
            assert!(
                validate_drop_if_contains(&[bad.to_string()]).is_err(),
                "{bad:?} should be rejected"
            );
        }
        assert!(validate_drop_if_contains(&["x".repeat(DROP_IF_CONTAINS_MAX_BYTES + 1)]).is_err());
        let too_many: Vec<String> = (0..=DROP_IF_CONTAINS_MAX_ITEMS)
            .map(|i| format!("needle-{i}"))
            .collect();
        assert!(validate_drop_if_contains(&too_many).is_err());
        assert!(validate_drop_if_contains(&["dup".to_string(), "dup".to_string()]).is_err());
    }

    #[test]
    fn the_new_multiline_modes_are_in_the_contract_vocabulary() {
        // 网关装载期与 agentd 配置校验都看这份词表：漏一项，目录里写了也会被当成非法。
        for mode in ["mysql_slow", "postgres_csv"] {
            assert!(MULTILINE_MODES.contains(&mode), "{mode}");
        }
        // 新读法一律要求能力声明（`none` 除外）。
        let old_agent = LogsCapabilities {
            multiline_modes: vec!["none".to_string(), "indented".to_string()],
            ..full_logs()
        };
        assert_eq!(
            missing(
                "FileGlob",
                "/var/log/postgresql/x.csv",
                "postgres_csv",
                &old_agent
            ),
            Some(vec!["multiline_modes:postgres_csv".to_string()])
        );
        assert_eq!(
            missing(
                "FileGlob",
                "/var/log/mysql/slow.log",
                "mysql_slow",
                &old_agent
            ),
            Some(vec!["multiline_modes:mysql_slow".to_string()])
        );
    }

    #[test]
    fn an_exporter_id_outside_the_vocabulary_is_reported_as_missing() {
        // 写错 ID / 未来 ID 遇到旧 agent：都是“没声明这个能力”，不能当成“不归我管”放过 ——
        // 放过的后果是派下去、agent 回 unsupported。
        assert_eq!(
            missing("Exporter", "journalctl-kernelx", "none", &full_logs()),
            Some(vec!["exporters:journalctl-kernelx".to_string()])
        );
        // 带 `arg` 也一样：只按 ID 判。
        assert_eq!(
            missing("Exporter", "dmesg:panic", "none", &old_style_logs()),
            Some(vec!["exporters:dmesg".to_string()])
        );
    }

    /// 只声明了“显式路径 + 任何导出器”这类旧能力的 agent。
    fn old_style_logs() -> LogsCapabilities {
        LogsCapabilities {
            file_inputs: vec![FILE_INPUT_ABSOLUTE_PATH.to_string()],
            exporters: vec!["smartctl".to_string()],
            ..LogsCapabilities::default()
        }
    }

    #[test]
    fn capability_checks_say_nothing_about_things_they_do_not_own() {
        // 指标不走 logs 能力；不可执行的形态由 `is_executable_source` 说，不重复扣帽子。
        assert_eq!(missing("MetricInterval", "15s", "none", &full_logs()), None);
        assert_eq!(missing("FileGlob", "~/a/*.log", "none", &full_logs()), None);
        assert_eq!(
            missing("UnifiedLogPredicate", "syspolicyd", "none", &full_logs()),
            None
        );
    }

    #[test]
    fn a_fully_declared_agent_is_missing_nothing_the_contract_calls_executable() {
        // 防漂：契约说“能执行”的每一种形态，声明了全集就必须不报缺。
        // 新增一个导出器 ID / 一种 multiline 模式 / 一个过滤维度而忘了加到常量里，这条会红。
        let full = full_logs();
        for target in ["/var/log/a.log", "/var/log/containers/*.log"] {
            assert_eq!(
                missing("FileGlob", target, "none", &full),
                Some(Vec::new()),
                "{target}"
            );
        }
        for mode in MULTILINE_MODES {
            assert_eq!(
                missing("FileGlob", "/var/log/a.log", mode, &full),
                Some(Vec::new()),
                "{mode}"
            );
        }
        for id in EXPORTER_IDS {
            assert_eq!(
                missing("Exporter", id, "none", &full),
                Some(Vec::new()),
                "{id}"
            );
        }
        // 每一个过滤维度都真的能“声明全了就不报缺”。
        let filters = ContainerFilters {
            namespace_in: vec!["prod".to_string()],
            pod_not_in: vec!["debug-pod".to_string()],
        };
        assert_eq!(
            filters.used_dimensions(),
            CONTAINER_FILTERS,
            "`used_dimensions` 与 `CONTAINER_FILTERS` 漂了"
        );
        assert_eq!(
            missing_log_capabilities(
                &container_source("/var/log/containers/*.log", filters),
                &full
            ),
            Some(Vec::new())
        );
    }

    // ── 工作参数（`spec`）的编码 ──

    fn spec() -> WorkSpec {
        WorkSpec {
            units: vec![
                WorkSpecUnit {
                    unit_id: "mac-host-metrics".to_string(),
                    capability: "collect_metrics".to_string(),
                    rule_ref: "agent_uplink".to_string(),
                    requires_privilege: "none".to_string(),
                    sources: vec![WorkSpecSource {
                        kind: "MetricInterval".to_string(),
                        target: "15s".to_string(),
                        multiline: "none".to_string(),
                        exclude_globs: Vec::new(),
                        filters: ContainerFilters::default(),
                        drop_if_contains: Vec::new(),
                    }],
                },
                WorkSpecUnit {
                    unit_id: "mac-privacy-tcc".to_string(),
                    capability: "collect_logs".to_string(),
                    rule_ref: "macos/tcc".to_string(),
                    requires_privilege: "fda".to_string(),
                    sources: vec![
                        WorkSpecSource {
                            kind: "Exporter".to_string(),
                            target: "sqlite-snapshot(TCC.db)".to_string(),
                            multiline: "none".to_string(),
                            exclude_globs: Vec::new(),
                            filters: ContainerFilters::default(),
                            drop_if_contains: Vec::new(),
                        },
                        WorkSpecSource {
                            kind: "FileGlob".to_string(),
                            // 显式单路径：单文件采集走 `startup_position` / 检查点的单文件语义。
                            target: "/var/log/tccd.log".to_string(),
                            // 多行格式是这条日志的固有属性，跟着来源走。
                            multiline: "indented".to_string(),
                            exclude_globs: Vec::new(),
                            filters: ContainerFilters::default(),
                            drop_if_contains: Vec::new(),
                        },
                    ],
                },
            ],
        }
    }

    #[test]
    fn spec_round_trips_and_reports_what_agentd_can_do() {
        let encoded = spec().encode().expect("encode");
        let decoded = WorkSpec::parse(&encoded).expect("parse");
        assert_eq!(decoded, spec());

        assert!(decoded.collects_logs());
        // 取了最密的周期：要得最急的才是上送频率的下界。
        assert_eq!(decoded.metric_interval_seconds(), Some(15));
        // 可本地 tail 的来源挑得出来，接不了的来源也报得出来（而不是当没看见）。
        let files = decoded.units[1].file_sources();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].target, "/var/log/tccd.log");
        assert_eq!(files[0].multiline, "indented");
        assert_eq!(decoded.units[1].unsupported_sources().len(), 1);
        assert_eq!(decoded.units[1].unsupported_sources()[0].kind, "Exporter");
    }

    #[test]
    fn a_glob_file_source_is_collectable_but_recursive_globs_are_not() {
        // W1 起：单层 glob 是采集端能执行的来源（容器日志就靠它）。
        let mut unit = WorkSpecUnit {
            unit_id: "linux-container-stdout".to_string(),
            capability: "collect_logs".to_string(),
            rule_ref: String::new(),
            requires_privilege: "root".to_string(),
            sources: vec![WorkSpecSource {
                kind: "FileGlob".to_string(),
                target: "/var/log/containers/*.log".to_string(),
                multiline: "none".to_string(),
                exclude_globs: vec!["/var/log/containers/*_kube-system_*.log".to_string()],
                filters: ContainerFilters::default(),
                drop_if_contains: Vec::new(),
            }],
        };
        assert_eq!(unit.file_sources().len(), 1, "它是一条文件来源");
        assert!(unit.unsupported_sources().is_empty());
        assert!(unit.has_executable_source());

        // `**` 仍不可执行：递归展开没实现，接受它就等于承诺一个采不到的面。
        unit.sources[0].target = "/var/log/**/*.log".to_string();
        assert_eq!(unit.unsupported_sources().len(), 1);
        assert!(!unit.has_executable_source());
    }

    #[test]
    fn exclude_globs_round_trip_and_stay_out_of_the_wire_when_empty() {
        let mut source = WorkSpecSource {
            kind: "FileGlob".to_string(),
            target: "/var/log/containers/*.log".to_string(),
            multiline: "none".to_string(),
            exclude_globs: Vec::new(),
            filters: ContainerFilters::default(),
            drop_if_contains: Vec::new(),
        };
        // 空排除项不进 JSON：旧 agent 的 `WorkSpecSource` 是 `deny_unknown_fields` 的。
        assert_eq!(
            serde_json::to_string(&source).expect("serialize"),
            r#"{"kind":"FileGlob","target":"/var/log/containers/*.log","multiline":"none"}"#
        );

        source.exclude_globs = vec!["/var/log/containers/*_kube-system_*.log".to_string()];
        let json = serde_json::to_string(&source).expect("serialize");
        assert_eq!(
            serde_json::from_str::<WorkSpecSource>(&json).expect("parse"),
            source
        );
        // 老网关发来的来源没有这两个字段：默认必须能解出来。
        let old = r#"{"kind":"FileGlob","target":"/a/*"}"#;
        let parsed: WorkSpecSource = serde_json::from_str(old).expect("parse legacy source");
        assert!(parsed.exclude_globs.is_empty());
        assert!(parsed.filters.is_empty());
        assert_eq!(parsed.multiline, "none");
    }

    #[test]
    fn container_filters_round_trip_and_stay_out_of_the_wire_when_empty() {
        let mut source = WorkSpecSource {
            kind: "FileGlob".to_string(),
            target: "/var/log/containers/*.log".to_string(),
            multiline: "none".to_string(),
            exclude_globs: Vec::new(),
            filters: ContainerFilters::default(),
            drop_if_contains: Vec::new(),
        };
        // 空过滤器不进 JSON（与 exclude_globs 同一条纪律：旧 agent 是 deny_unknown_fields）。
        let json = serde_json::to_string(&source).expect("serialize");
        assert!(!json.contains("filters"), "{json}");

        source.filters = ContainerFilters {
            namespace_in: vec!["prod".to_string()],
            pod_not_in: vec!["debug-pod".to_string()],
        };
        let json = serde_json::to_string(&source).expect("serialize");
        assert!(
            json.contains(r#""filters":{"namespace_in":["prod"],"pod_not_in":["debug-pod"]}"#),
            "{json}"
        );
        assert_eq!(
            serde_json::from_str::<WorkSpecSource>(&json).expect("parse"),
            source
        );

        // 只给一半也要能解：另一侧默认空。
        let half = r#"{"kind":"FileGlob","target":"/var/log/containers/*.log","filters":{"namespace_in":["prod"]}}"#;
        let parsed: WorkSpecSource = serde_json::from_str(half).expect("parse half filters");
        assert_eq!(parsed.filters.namespace_in, vec!["prod".to_string()]);
        assert!(parsed.filters.pod_not_in.is_empty());
    }

    #[test]
    fn an_explicitly_empty_filters_object_is_not_serialized() {
        // 策展/页面可能显式写一个空对象（`filters = {}`）：它仍然等于“不过滤”，
        // 不能在线上多出一个键 —— 旧 agent 是 `deny_unknown_fields` 的。
        let source = WorkSpecSource {
            kind: "FileGlob".to_string(),
            target: "/var/log/containers/*.log".to_string(),
            multiline: "none".to_string(),
            exclude_globs: Vec::new(),
            filters: ContainerFilters {
                namespace_in: Vec::new(),
                pod_not_in: Vec::new(),
            },
            drop_if_contains: Vec::new(),
        };
        let json = serde_json::to_string(&source).expect("serialize");
        assert!(!json.contains("filters"), "{json}");
    }

    #[test]
    fn container_filters_keep_values_verbatim() {
        // 名字**不做归一化**（不折叠大小写、不去前后缀）：k8s 名字永远小写，
        // 归一化反而会把写错的值当成能用。逐字保存、逐字比较（比较在 agentd）。
        let filters = ContainerFilters {
            namespace_in: vec!["Prod".to_string()],
            pod_not_in: vec!["debug-pod".to_string()],
        };
        let json = serde_json::to_string(&filters).expect("serialize");
        let parsed: ContainerFilters = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed.namespace_in, vec!["Prod".to_string()]);
        assert_eq!(parsed.pod_not_in, vec!["debug-pod".to_string()]);
    }

    #[test]
    fn container_filters_reject_unknown_keys() {
        // `deny_unknown_fields`：拼错一个键（`namespaces`）不能静默当空过滤器 ——
        // 那会让策展人以为「只采 prod」，实际全采。
        assert!(serde_json::from_str::<ContainerFilters>(r#"{"namespaces":["prod"]}"#).is_err());
        assert!(serde_json::from_str::<ContainerFilters>(r#"{"namespace_in":["prod"]}"#).is_ok());
    }

    #[test]
    fn container_filters_reject_values_that_can_never_match() {
        let ok = |namespace: &[&str], pods: &[&str]| ContainerFilters {
            namespace_in: namespace.iter().map(|v| v.to_string()).collect(),
            pod_not_in: pods.iter().map(|v| v.to_string()).collect(),
        };

        assert!(
            ok(&["prod", "kube-system"], &["debug-pod"])
                .validate()
                .is_ok()
        );
        assert!(ok(&[], &[]).is_empty());
        // 这些一定对不上（过滤器存的是**精确名字**，不是 pattern）：错在最早的入口就说。
        assert!(ok(&[""], &[]).validate().is_err());
        assert!(ok(&[" prod"], &[]).validate().is_err());
        assert!(ok(&["prod*"], &[]).validate().is_err());
        assert!(ok(&[], &["debug?pod"]).validate().is_err());
        assert!(ok(&[&"x".repeat(254)], &[]).validate().is_err());
    }

    #[test]
    fn a_source_without_multiline_decodes_as_single_line() {
        // 老网关发来的 spec 没有 `multiline`：默认必须是一行一条。
        // 取错默认值会把多行日志**粘**成一条 —— 那是无声的内容损坏，不是格式问题。
        let json = r#"{"units":[{"unit_id":"u","capability":"collect_logs","sources":[{"kind":"FileGlob","target":"/a/*"}]}]}"#;
        let spec = WorkSpec::parse(json).expect("parse");
        assert_eq!(spec.units[0].sources[0].multiline, "none");
        // 再编码出去会把默认值写实：新网关发的 spec 字段总是齐的。
        assert_eq!(
            spec.encode().expect("encode"),
            r#"{"units":[{"unit_id":"u","capability":"collect_logs","rule_ref":"","requires_privilege":"","sources":[{"kind":"FileGlob","target":"/a/*","multiline":"none"}]}]}"#
        );
    }

    #[test]
    fn a_broken_spec_is_an_error_not_an_empty_work() {
        // 参数坏了与「没事可做」是两回事：前者要人去看，后者什么都不用做。
        assert!(WorkSpec::parse("mac-host-metrics").is_err());
        assert!(WorkSpec::parse("{").is_err());
        // 空工作本身是合法的（能表示「这个面暂时没东西可采」）。
        assert!(WorkSpec::parse(r#"{"units":[]}"#).unwrap().units.is_empty());
        assert_eq!(
            WorkSpec::parse(r#"{"units":[]}"#)
                .unwrap()
                .metric_interval_seconds(),
            None
        );
    }

    #[test]
    fn interval_parsing_accepts_seconds_and_minutes_only() {
        assert_eq!(parse_interval_seconds("15s"), Some(15));
        assert_eq!(parse_interval_seconds(" 5s "), Some(5));
        assert_eq!(parse_interval_seconds("5m"), Some(300));
        // 认不出来的回 None（调用方回退到自己的默认值），而不是静默当 0。
        assert_eq!(parse_interval_seconds("15"), None);
        assert_eq!(parse_interval_seconds("0s"), None);
        assert_eq!(parse_interval_seconds(""), None);
    }

    #[test]
    fn interval_parsing_never_overflows_on_huge_values() {
        // 契约是「解析不了返回 None」，不是 panic：溢出在 debug 下 panic、release 下回绕成一个
        // 错误的间隔 —— 两个后果都不能接受。这是两侧都要解析的字节，输入可能来自远端。
        assert_eq!(parse_interval_seconds("922337203685477580m"), None);
        assert_eq!(parse_interval_seconds("153722867280912931m"), None);
        // 秒分支本来就靠 parse 失败兜底。
        assert_eq!(parse_interval_seconds("99999999999999999999s"), None);
        // 合法但很大的分钟值仍要算得出来（不误伤）。
        assert_eq!(parse_interval_seconds("600m"), Some(36_000));
    }

    #[test]
    fn metric_interval_ignores_unparseable_targets_instead_of_treating_them_as_zero() {
        // 认不出的周期不能静默当 0（0 会被当成「无限密」），而要如实跳过，只取能解析的最密值。
        let unit = |target: &str| WorkSpecUnit {
            unit_id: "u".to_string(),
            capability: "collect_metrics".to_string(),
            rule_ref: String::new(),
            requires_privilege: String::new(),
            sources: vec![WorkSpecSource {
                kind: "MetricInterval".to_string(),
                target: target.to_string(),
                multiline: "none".to_string(),
                exclude_globs: Vec::new(),
                filters: ContainerFilters::default(),
                drop_if_contains: Vec::new(),
            }],
        };
        let mixed = WorkSpec {
            units: vec![unit("garbage"), unit("30s")],
        };
        assert_eq!(mixed.metric_interval_seconds(), Some(30));

        let all_bad = WorkSpec {
            units: vec![unit("nope")],
        };
        assert_eq!(all_bad.metric_interval_seconds(), None);
    }
}
