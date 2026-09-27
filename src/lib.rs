//! Versioned contract objects shared by edge and center components.

pub mod action_plan;
pub mod action_result;
pub mod agent_config;
pub mod agent_state;
pub mod agent_uplink;
pub mod capability_report;
pub mod discovery;
pub mod discovery_policy;
pub mod enrollment;
pub mod execution_state;
pub mod exporter;
pub mod fact_summary;
pub mod gateway;
pub mod ingest;
pub mod local_work;
pub mod telemetry_record;
pub mod work;

pub const API_VERSION_V1: &str = "v1";
pub const SCHEMA_VERSION_V1: &str = "v1";
