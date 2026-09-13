//! Execution local state contract types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionRuntimeContext {
    pub execution_id: String,
    pub spawned_at: String,
    pub deadline_at: Option<String>,
    pub agent_id: String,
    pub node_id: String,
    pub workdir: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionProgressState {
    pub execution_id: String,
    pub action_id: String,
    pub state: String,
    pub updated_at: String,
    pub step_id: Option<String>,
    pub attempt: Option<u32>,
    pub reason_code: Option<String>,
    pub detail: Option<String>,
}
