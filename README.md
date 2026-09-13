# wist-contracts

Versioned contract and schema objects shared by edge and center components.

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/rustc-1.85+-orange.svg)](#)

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
| `capability_report` | Agent capability declarations.                       |
| `discovery`         | Discovered resources and target views.               |
| `enrollment`        | Enrollment and credential objects.                   |
| `execution_state`   | Execution local state objects.                       |
| `exporter`          | Export / delivery contracts.                         |
| `gateway`           | Gateway-facing request / response objects.           |
| `ingest`            | Telemetry ingestion contracts.                       |
| `telemetry_record`  | Normalized telemetry records.                        |

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
