# wist-contracts

Versioned contract and schema objects shared by edge and center components.

[![crates.io](https://img.shields.io/crates/v/wist-contracts.svg)](https://crates.io/crates/wist-contracts)
[![docs.rs](https://img.shields.io/docsrs/wist-contracts/latest.svg)](https://docs.rs/wist-contracts)
[![Downloads](https://img.shields.io/crates/d/wist-contracts.svg)](https://crates.io/crates/wist-contracts)
[![MSRV](https://img.shields.io/badge/rustc-1.85+-orange.svg)](#)
[![CI](https://github.com/dayu-sec/wist-contracts/actions/workflows/ci.yml/badge.svg)](https://github.com/dayu-sec/wist-contracts/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/dayu-sec/wist-contracts/branch/main/graph/badge.svg)](https://codecov.io/gh/dayu-sec/wist-contracts)
[![dependency status](https://deps.rs/repo/github/dayu-sec/wist-contracts/status.svg)](https://deps.rs/repo/github/dayu-sec/wist-contracts)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

`wist-contracts` is the single source of truth for the serialized objects exchanged between the
edge ([`wist-agentd`](../wist-agentd)) and the center (gateway / center). Every type is a plain
`serde` struct so it can round-trip over JSON and stay versioned independently of any single
consumer.

## Modules

| Module              | Purpose                                              |
| ------------------- | ---------------------------------------------------- |
| `action_plan`       | The `ActionPlan` executed by `wist-exec`.            |
| `action_result`     | The result of an execution.                          |
| `agent_config`      | The `wist-agentd` runtime configuration (`agentd.toml`). |
| `agent_state`       | Agent runtime state (identity, credentials, mode).   |
| `agent_uplink`      | Data-plane uplink grant (control plane) and the agent-reported effective uplink state. |
| `capability_report` | Agent capability declarations.                       |
| `discovery`         | Discovered resources and target views.               |
| `discovery_policy`  | Curated discovery aspect policy sets (version + published_at). |
| `enrollment`        | Enrollment and credential objects.                   |
| `execution_state`   | Execution local state objects.                       |
| `exporter`          | Export / delivery output envelope.                   |
| `fact_summary`      | Fact content canonicalization and its idempotency digest. |
| `gateway`           | Gateway-facing request / response objects.           |
| `ingest`            | Telemetry ingestion contracts.                       |
| `local_work`        | Agent-reported local work view (`state/work.json` subset). |
| `telemetry_record`  | Normalized telemetry records.                        |
| `work`              | Work grants (standing / one-shot), specs, and results. |

Version markers are exposed as constants:

```rust
wist_contracts::API_VERSION_V1;    // "v1"
wist_contracts::SCHEMA_VERSION_V1; // "v1"
```

## Example

```rust
use wist_contracts::action_plan::ActionPlanContract;

let plan: ActionPlanContract = serde_json::from_str(json)?;
```

## Related crates

- [`wist-validate`](../wist-validate) — validates these contracts.
- [`wist-shared`](../wist-shared) — helpers used to read and write them.
- [`wist-agentd`](../wist-agentd) — the primary edge consumer.

## License

[Apache-2.0](LICENSE)
