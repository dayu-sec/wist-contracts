//! 事实摘要的**规范化**与内容摘要（幂等键）。
//!
//! ## 为什么放在契约 crate
//!
//! agentd 与网关必须算出**同一个**「内容没变」的判据。两侧各写一份的话，
//! 规范化一旦不一致就会出现「一边认为没变、另一边认为变了」，或者更糟 ——
//! 一边认为变了、另一边**永远**认为没变（静默漏报，且没有任何一层能发现）。
//! 所以规范化与摘要只有一个实现，两侧都调它。
//!
//! ## 为什么网关要自己重算、不信 agent 的声明
//!
//! `content_digest` 是**变更检测的幂等键**：一旦它是错的，后果是「永远不更新」而不是
//! 「多发一次」。而 agent 侧的算法可能因为版本、缺陷或损坏的文件而退化（例如退化成常量），
//! 那时若网关仍拿 agent 的声明判重，就会把所有上报都当重复、静默停在旧内容上。
//! 所以：**判重键由网关从收到的内容自己算**；wire 上的 `content_digest` 只作参考，
//! 不一致时记告警（可能只是版本偏差，**不是**非法输入，不该据此拒收）。
//!
//! ## 摘要版本前缀
//!
//! 前缀 `fact-v1` 表达的是**规范化算法/字段集的版本**。改字段集（例如以后纳入容器或
//! 资源画像）时必须 bump 它 —— 否则新旧两侧对同一台机器会给出不同的「没变」，
//! 而这个差异是静默的。带上前缀后，版本切换会强制一次重报，而不是静默错判。

use serde::Serialize;
use sha2::{Digest, Sha256};

/// 规范化算法 / 字段集的版本（见模块注释）。
pub const FACT_DIGEST_VERSION: &str = "fact-v1";

/// 事实的**内容**部分（判据、幂等键的输入）。
///
/// 刻意**不含** `revision` / `observed_at` / `process_count`：
/// 前两者每轮观测都变，后者随无关进程生灭一直动 —— 放进去幂等就失效了。
/// 摘要表达的是「内容」，不是「哪一次采的」。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FactContent {
    pub os: String,
    pub arch: String,
    pub process_executables: Vec<String>,
    pub packages: Vec<String>,
    pub listen_ports: Vec<String>,
}

impl FactContent {
    pub fn new(
        os: impl Into<String>,
        arch: impl Into<String>,
        process_executables: Vec<String>,
        packages: Vec<String>,
        listen_ports: Vec<String>,
    ) -> Self {
        Self {
            os: os.into(),
            arch: arch.into(),
            process_executables,
            packages,
            listen_ports,
        }
    }

    /// 内容摘要（形如 `fact-v1:sha256:<hex>`）。
    ///
    /// 归一化（去重 + 定序）放在**这个函数内部**，而不是只靠调用方：
    /// 不变式要由主张它的函数来守 —— 否则调用方传进来的顺序或重复项一变，
    /// 幂等键就静默失真（"内容没变就不发"不再成立）。
    pub fn content_digest(&self) -> String {
        let canonical = CanonicalContent {
            os: &self.os,
            arch: &self.arch,
            process_executables: normalized(&self.process_executables),
            packages: normalized(&self.packages),
            listen_ports: normalized(&self.listen_ports),
        };
        // 对 &str / Vec<String> 序列化不可能失败；不用 unwrap_or_default ——
        // 那会把"序列化失败"变成"空输入的常量摘要"，等于"内容永远没变"，上报永久停摆。
        let bytes =
            serde_json::to_vec(&canonical).expect("fact content serialization is infallible");
        let hash = Sha256::digest(&bytes);
        let hex: String = hash.iter().map(|byte| format!("{byte:02x}")).collect();
        format!("{FACT_DIGEST_VERSION}:sha256:{hex}")
    }
}

/// 定序后的规范视图（字段名与顺序固定 → 同一内容必得同一字节序列）。
#[derive(Serialize)]
struct CanonicalContent<'a> {
    os: &'a str,
    arch: &'a str,
    process_executables: Vec<String>,
    packages: Vec<String>,
    listen_ports: Vec<String>,
}

fn normalized(values: &[String]) -> Vec<String> {
    let mut sorted: Vec<String> = values.to_vec();
    sorted.sort();
    sorted.dedup();
    sorted
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content(executables: &[&str]) -> FactContent {
        FactContent::new(
            "macos",
            "arm64",
            executables.iter().map(|value| value.to_string()).collect(),
            Vec::new(),
            Vec::new(),
        )
    }

    #[test]
    fn digest_is_stable_and_versioned() {
        let digest = content(&["/usr/bin/xcodebuild"]).content_digest();
        assert!(digest.starts_with("fact-v1:sha256:"), "{digest}");
        assert_eq!(digest.len(), "fact-v1:sha256:".len() + 64);
        // 同一内容重复计算必须一致（否则幂等键本身就不成立）。
        assert_eq!(digest, content(&["/usr/bin/xcodebuild"]).content_digest());
    }

    #[test]
    fn digest_normalizes_order_and_duplicates() {
        let base = content(&["b", "a"]).content_digest();
        assert_eq!(base, content(&["a", "b"]).content_digest());
        assert_eq!(base, content(&["b", "a", "a"]).content_digest());
    }

    #[test]
    fn digest_changes_with_content() {
        let base = content(&["a"]).content_digest();
        assert_ne!(base, content(&["a", "b"]).content_digest());
        // os/arch 变了摘要必变（平台判据切换一定会触发重报）。
        let mut other = content(&["a"]);
        other.os = "linux".to_string();
        assert_ne!(base, other.content_digest());
    }

    #[test]
    fn revision_and_process_count_are_outside_the_digest() {
        // 这两者不在 FactContent 里 —— 编译期就保证了它们进不了摘要。
        let content = content(&["a"]);
        let before = content.content_digest();
        let recomputed = FactContent::new(
            content.os.clone(),
            content.arch.clone(),
            content.process_executables.clone(),
            content.packages.clone(),
            content.listen_ports.clone(),
        );
        assert_eq!(before, recomputed.content_digest());
    }

    #[test]
    fn packages_and_listen_ports_are_part_of_the_digest() {
        // 这两个字段同样进内容摘要（用途判据的输入）：任一变则重报。
        let base = FactContent::new("linux", "x86_64", Vec::new(), Vec::new(), Vec::new())
            .content_digest();
        let with_package = FactContent::new(
            "linux",
            "x86_64",
            Vec::new(),
            vec!["nginx".to_string()],
            Vec::new(),
        )
        .content_digest();
        let with_port = FactContent::new(
            "linux",
            "x86_64",
            Vec::new(),
            Vec::new(),
            vec!["443".to_string()],
        )
        .content_digest();
        assert_ne!(base, with_package);
        assert_ne!(base, with_port);
        assert_ne!(with_package, with_port);
    }

    #[test]
    fn an_empty_content_digest_is_stable_and_well_formed() {
        let digest = FactContent::new("", "", Vec::new(), Vec::new(), Vec::new()).content_digest();
        assert!(digest.starts_with("fact-v1:sha256:"));
        assert_eq!(digest.len(), "fact-v1:sha256:".len() + 64);
        assert_eq!(
            digest,
            FactContent::new("", "", Vec::new(), Vec::new(), Vec::new()).content_digest()
        );
    }
}
