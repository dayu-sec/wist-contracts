//! Execution local state contract types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeContext {
    pub execution_id: String,
    pub spawned_at: String,
    pub deadline_at: Option<String>,
    pub agent_id: String,
    pub node_id: String,
    pub workdir: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgressState {
    pub execution_id: String,
    pub action_id: String,
    pub state: String,
    pub updated_at: String,
    pub step_id: Option<String>,
    pub attempt: Option<u32>,
    pub reason_code: Option<String>,
    pub detail: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_context_round_trips_and_rejects_unknown_fields() {
        let context = RuntimeContext {
            execution_id: "exec-1".to_string(),
            spawned_at: "2026-09-27T00:00:00Z".to_string(),
            deadline_at: Some("2026-09-27T00:05:00Z".to_string()),
            agent_id: "agent-1".to_string(),
            node_id: "node-1".to_string(),
            workdir: "/var/lib/wist/work/exec-1".to_string(),
        };
        let json = serde_json::to_string(&context).expect("encode");
        let back: RuntimeContext = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, context);

        let mutated = json.replacen('{', "{\"extra\":1,", 1);
        assert!(serde_json::from_str::<RuntimeContext>(&mutated).is_err());
    }

    #[test]
    fn progress_state_round_trips_with_optional_step_and_attempt() {
        let progress = ProgressState {
            execution_id: "exec-1".to_string(),
            action_id: "act-1".to_string(),
            state: "running".to_string(),
            updated_at: "2026-09-27T00:00:01Z".to_string(),
            step_id: Some("step-2".to_string()),
            attempt: Some(3),
            reason_code: None,
            detail: None,
        };
        let json = serde_json::to_string(&progress).expect("encode");
        let back: ProgressState = serde_json::from_str(&json).expect("decode");
        assert_eq!(back, progress);

        // 手写的最小进度（无步级断点）也必须能解析：Option 缺省即可。
        let minimal = r#"{"execution_id":"e","action_id":"a","state":"running",
                          "updated_at":"t"}"#;
        let decoded: ProgressState = serde_json::from_str(minimal).expect("decode");
        assert_eq!(decoded.step_id, None);
        assert_eq!(decoded.attempt, None);
    }
}
