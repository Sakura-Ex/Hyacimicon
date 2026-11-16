# Single-sample run foundation

`run-core` composes the existing `battle-core` with a small run environment.
A selected encounter produces one sampled battle result. That actual result
drives explicit resource settlement, the next node and a training transition.
Repeated same-encounter sampling remains an offline evaluation operation.

This is a framework candidate awaiting team feedback. The executable examples
use synthetic rules and effective battle inputs. Complete live-game settlement,
account conversion, shops, investment, model training, Python bindings and the
Tauri UI are later work. Review the [architecture questions](../../docs/modules/run-simulation/README.md#框架评审与继续条件)
before extending this foundation.

## Build and run

Run from `core/run/`; Cargo configuration, the lockfile and the Rust toolchain
are owned by this module. The dependency on `../battle` reuses its actual engine.

```text
cd core/run
cargo test --locked
cargo run --locked --bin hcwa-run -- simulate-run examples/two-battles.json 17
cargo build --release --locked
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
```

The CLI runs one in-memory episode using run ID `cli-run`. The example starts
with HP 30 and gold 10. A defeat applies explicit synthetic HP -7/gold +2 and
continues; a later victory grants gold +5. The expected final resources are
HP 23 and gold 17, with both transitions retained. These deltas are fixture
inputs, not Currency Wars formulas.

Output includes the final observation, battle attempts, settlement receipts and
training transitions. It is an environment/replay report, not policy input.
The CLI stops on completion, run defeat, interruption or missing settlement;
it never rerolls a pending encounter. Paths are relative to the current working
directory. Keep generated output outside tracked source.

If a step fails after the environment is created, the CLI still writes its
partial report to stdout, writes the error to stderr and exits with code 2.
Earlier transitions and any saved pending battle remain in that report; the
rejected settlement does not change resources. Invalid input that prevents
environment creation produces an error without a report.

## Ownership and calls

| API | Contract |
| --- | --- |
| `RunEnv::new` | Validate an explicit `RunSpec`, run identity and master seed |
| `observe` | Return current policy-visible resources, node, revision and legal actions |
| `sample` | Execute the real battle engine once and save its result; exact request replay reuses the saved attempt |
| `settle` | Apply rules to the saved actual outcome atomically, producing a receipt and transition |
| `step` | Sample once; settle completed outcomes with an applicable rule, retain completed outcomes with missing rules, or mark incomplete execution as interrupted |
| `report` | Export replay/experience records separately from the policy observation |

The only current legal action is `FightCurrentNode` while the run is `Ready`;
other states expose no policy actions. Shops and preparation choices are later
action extensions. `account_config_id` is a required caller-supplied identity
label, not a claim that account conversion or validation has run.

Run identity, node identity, request identity and state revision prevent stale
or conflicting commands from changing state. Repeating a committed settlement
must not change resources or produce a second transition. Missing rules can be
supplied later through `settle` without calling the battle engine again.
Automatic settlement IDs avoid all IDs already used for sampling or settlement;
callers do not need to reserve an ID prefix.

The run owns HP, maximum HP, gold, crystals and node progress. The battle owns
unit HP, energy, AV and combat effects. Each node has an explicit initial battle
scenario; this foundation does not infer unit carryover or a game reset policy
from the run resource balances. Gold and crystals use checked integer balances;
invalid settlement is rejected before any state mutation.

Only `synthetic_explicit_v1` settlement rules are implemented. Their resulting HP, computed as `hp + delta.hp`,
is clamped to `[0, max_hp]`, with requested and applied deltas recorded.
Maximum HP stays fixed in this profile. Currency underflow/overflow is rejected.
Victory, defeat and draw branches are explicit; a missing branch is unknown,
not an implicit zero reward. Rules record their identity and source. Implement
a separately documented rule profile before using real game settlements.

## Diagnostics and training

Diagnostics preserve the engine's outcome and termination. They compute
`used = elapsed_av + penalty_av`, then `margin = max_av - used`.
The displayed remainder is `max(0, margin)`; overrun and the signed margin
remain available. Victory still wins the engine's terminal-priority check even
when revival penalties consume the remaining budget in the final action.

`failure_reason` reports a known defeat termination. It does not infer why a
team or policy was poor. Cancellation and action limits are incomplete outcomes;
they interrupt the environment without applying a defeat rule. A draw requires
its own explicit settlement branch.

Every settled defeat is retained, including defeats after which the episode
continues. Pending results remain in attempt records until settlement is
possible. The scalar reward is `None`: no training reward formula is defined
by this framework. Full reports include seeds and traces for replay, while
`RunObservation` excludes seeds, future nodes, full scenarios and traces.

Independent `RunEnv` instances keep separate mutable state. Node seeds derive
from the master seed, run ID, node index and attempt index; replay keeps all four
unchanged. Independent episodes use distinct run IDs or master seeds. Master and
battle seeds are decimal strings in JSON; TypeScript must preserve those strings.
The report retains both run and per-battle provenance, including mixed snapshots.
Parallel training will drive multiple such episodes, consuming one actual
result at each selected encounter. This module does not implement a parallel
worker pool or learning algorithm. `battle::batch` remains reusable for offline
candidate comparisons and does not mutate this run.

## Validation scope

Tests exercise the actual battle engine through the run API: defeat continuation,
victory completion, terminal HP, pending settlement, duplicate/conflicting
requests, stale revisions, interruption, checked resource changes, observation
boundaries, independent environments and replay. The existing battle suite
continues to define `basic_v1` behavior.

Synthetic end-to-end execution verifies the framework contract. Game-rule
validation needs separately identified node evidence and effective configurations.
The [project roadmap](../../docs/project/roadmap.md) places team framework
acceptance before those extensions.
