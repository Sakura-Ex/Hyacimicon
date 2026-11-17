# Battle v2

This module executes explicit battle operations with remaining-distance scheduling,
Turn/Action boundaries, event listeners, insertion requests and per-cycle budgets.
It continues PR8's damage/heal/shield math, target/critical random streams, CLI and
batch implementation. It replaces the old `basic_v1` scheduler; old inputs must
select `battle_v2` and account for the changed rules described below.

## Runnable character scene

The [Anaxa S2 scenario](ANAXA_SCENARIO.md) now runs a complete explicit battle
through finite-queue replacement and named provisional run-HP settlement. It
naturally executes basic, skill, ultimate, disclosure bonus action and S2 pursuit
under one fixed strategy. Single runs and batches share the existing engine.
The reference kit is approximate: random weakness implantation, full bonus-skill
inheritance, hard control and other documented mechanics are omitted and can
change the result. This is not full native reproduction or arbitrary-roster support.

```text
cargo run -p battle-core --release --locked -- simulate core/battle/examples/anaxa-s2-full-battle.json 42
cargo run -p battle-core --release --locked -- batch core/battle/examples/anaxa-s2-full-battle.json 8 42
```

[Scenario details](ANAXA_SCENARIO.md) specify input fields, formula/order,
source versions and approximations. [Data requirements](DATA_REQUIREMENTS.md)
list evidence needed for future game alignment; no private tables are bundled.

## Run

The repository root defines the Cargo workspace, shared package metadata and
dependency versions. Its `Cargo.lock` and `rust-toolchain.toml` pin dependencies
and the Rust toolchain for both root and module commands. The workspace currently
contains only `core/battle`.

From the repository root:

```text
cargo test -p battle-core --locked
cargo build -p battle-core --release --locked
cargo run -p battle-core --release --locked -- simulate core/battle/examples/team-waves.json 7
```

Commands from this module directory remain valid and use the same workspace:

```text
cargo test --locked
cargo build --release --locked
cargo run --release --locked -- simulate examples/team-waves.json 7
```

The original public examples are artificial operation tests. The Anaxa example
adds a documented public-reference character approximation with developer panels.
Private game catalogs and player data are supplied separately, never bundled here. `validate FILE`, `simulate FILE SEED`, and
`batch FILE TRIALS SEED` retain their CLI argument structure. Seeds remain decimal
strings in results. The library entry points are `validate`, `simulate`,
`simulate_cancellable`, and `simulate_with_policy`.

Results include `provenance`, outcome/termination, elapsed AV, charged penalty AV,
current cycle and remaining cycle/total AV, final unit HP/shield/energy/distance/
speed/toughness, and optional ordered replay events. `Incomplete` denotes a safety
or cancellation stop, not a defeat. Explicit `mechanics.settlement` enables the
named provisional single-battle policy and reports progress, actual HP loss,
recovery, clipping and HP after. `run_hp_loss` remains null for legacy inputs and
Incomplete runs. Unit HP is not run HP. Combat `outcome`, minimum progress and
run survival remain distinct; HP > 0 never implies combat victory. The planned
`core/run` still owns persistent whole-run state; this module computes only this
battle's configured HP transition and does not create or persist an entire run.

## Scheduling and operations

- `mechanics.cycles` supplies successive AV allowances; its sum must equal
  `max_av`. An omitted list means one cycle of `max_av`.
- Each living unit holds one remaining distance. Waiting AV is distance/current
  speed; time advancement subtracts `delta * speed` from every living unit.
- The normal actor stays at distance zero until its TurnEnd, including inserted
  actions. `advance` changes the current distance (and has no effect on the normal
  actor at the top); `advance_next` records a distance change applied at TurnEnd.
  `speed` replaces current speed without changing remaining distance.
- `request` with `ultimate` carries an action ID. Queueing costs nothing. Entry
  revalidates the request and pays once before TurnStart listeners. `extra_turn`
  carries no action and chooses after TurnStart. `follow_up` executes an Action
  directly and emits no TurnStart/TurnEnd. The latter requires a `follow_up` action.
  Requests are FIFO; duplicate pending ultimates for one unit are coalesced.
- An Action executes its complete effect vector. Runtime Hit/Break/Death listeners
  can apply immediate effects or register later actions. No queued action runs
  inside another Action. Runtime dispatch is independent of replay limits.
  Damage and toughness recheck each recipient after preceding immediate callbacks.
  Only the hit that changes positive HP to zero dispatches Death and revival;
  revival permits a later hit to cause a new death. A dead owner receives its own
  Death event, while its listeners for other events remain inactive.
- `mechanics.triggers` binds owner, event, optional subject/action filters, uses
  (zero means unlimited), and effects. BattleStart/WaveStart/CycleEnd are global
  events with no unit subject; a subject filter on them is rejected. Other events
  use the acting unit, except Break/Death which use the affected unit. Optional
  event-based durations tick for every present owner on global events and only
  for the subject's own statuses on unit events. Future-wave owners start counting
  after entry. All applicable expirations settle before any listener of the event
  executes, including listeners that dispatch nested events. Shields retain their specified normal-turn
  expiry. `toughness` effects dispatch Break; elemental break damage/recovery must
  be explicitly implemented by supplied effects, not inferred from a scalar.
- Revival restores explicit HP and consumes only the current cycle's remaining
  allowance, clamped to zero. It preserves the clock and all unit distances.
- Terminal elimination is checked at the common boundary after complete actions
  and state-changing cycle events. Cycle exhaustion precedes safety cancellation.
- `ability_target` chooses the lowest-HP-fraction enemy once at Action entry. Every hit
  retains that identity; later hits stop if it dies. Existing `enemy_lowest_hp` and
  `enemy_random_taunt` explicitly reselect per hit; `enemy_random_uniform` selects
  uniformly from living enemies using the same seeded target stream. A full game policy's target
  selection/Retarget instructions need their own adapter.

`Policy` receives an immutable observation and the legal `Operation` list.
`Automatic` selects its first entry; closures use exactly the same interface.
Normal/extra-turn choices use declared action order. Automatic ultimate requests
use `ultimate_priority` keyed by unit ID, then stable unit/action order, and are
rechecked after each completed action.

Priority comparison direction, same-priority order, and cycle/action ties are not
claimed as measured game rules. The caller explicitly controls
`larger_priority_first` (default false) and `unit_before_cycle` (default false).
Unit-first ties drain zero-time due units/requests before the cycle event.

## Damage factor

`mechanics.ally_damage_factor` is applied once at the common damage settlement
point, after the reused formula and before shield absorption. Its neutral default
is 1.0. The owner-reported 2026-11-17 Currency Wars value is 0.3; set it explicitly.
Its exact underlying game property/multiplier group remains unresolved. Do not
also multiply skill ratios or enemy reduction by it. Source:
https://github.com/Sakura-Ex/hsr-currency-wars-ai/pull/8#issuecomment-6922807938

## Raw Config skill-chain compiler

```text
python tools/config_actions.py REQUEST.json COMPILED.json
python -m unittest discover -s tools -p "test_*.py"
```

The request provides `ability_files`, `entry`, explicit numeric `bindings`,
`active_points`, and resolved received `hit_energy`. The compiler traverses actual
Ability references and regular-skill predicates, lowers ordered damage, toughness,
hit energy and next-action shifts, and records every visited source pointer. It
removes only enumerated presentation tasks and preserves hit order. Postfix load,
constant and negation formats used by the exercised chain are supported; unknown
instructions, numeric modifiers, missing abilities/bindings or fields reject.
`Inverse` is respected. Numeric modifiers are never dropped as animations.
Ability callbacks execute only OnAdd and OnStart. Nonempty unsupported fields or
callbacks (including OnEnd) reject at their source pointer before lowering; empty
unsupported fields are recorded. TargetInfo, when present, must specify only
SkillTargetEntityList. Unused Modifier definitions are declarations; referenced
modifiers are validated by AddModifier.

Every handled regular-skill task now uses a closed field contract, including
presentation/wait/marker tasks. Camera, movement, target aliases, reference names,
predicates, delay parameters and numeric wrappers are checked recursively at
their consumed fields. Unknown fields (even empty payloads) and hidden task lists
reject at their exact JSON pointer. `WaitSecond` accepts the inspected `WaitTime`
field; an unrelated `Seconds` spelling is not inferred. `ModifyCurrentSkillDelayCost`
permits an omitted target under the existing self-delay contract or an explicit
Caster, never an ignored enemy target. `TriggerAbility` likewise requires the
selected explicit Caster reference contract. Preshow modifiers may only contain
the inspected preview structure, explicitly bound local values and optional
OnActionEnd/RemoveSelfModifier bookkeeping; unknown callbacks or fields reject.
The common numeric shape check also protects runtime expression lowering,
including all supplied literal/fixed/postfix operand wrappers.

Regular-context predicates select one branch using supplied active points and
the declared non-insert/caster-turn context. Both branch containers must be lists;
the excluded branch is recorded as `not_selected_declared_regular_context`, with
its pointer and task count, and is not claimed as compiled or validated native
logic. This preserves the selected actual regular-skill chains, whose excluded
insert-only branches require other mechanisms. Runtime conditional lowering
continues to check both executable branches. The field schemas cover selected
inspected source structures, not arbitrary native camera/ability formats; new
fields require an explicit supported interpretation before acceptance.

Explicit `CanTriggerLastKill` values other than boolean `true` reject. For the
legacy regular-skill compiler, an omitted value retains its documented direct-hit
contract and is recorded as an unresolved native default. The S2 pursued-hit
compiler requires explicit `true`; it does not treat this field as presentation.

This is **selected regular-skill-chain execution**, not a complete roster importer.
Provide resistance already resolved for the emitted damage element. The local
acceptance run resolves GridFight Seele Skill01 through its Advanced parent,
reads SkillID 1110201 level 1 and PointID 11102103, and executes it against
MonsterSkillID 801101001. Both use supplied controlled panels. Full selected kit
imports are also attempted and rejected at unsupported passive/skill operations.
These controlled results must not be presented as a real roster win probability.

## Explicit runtime state

`mechanics.dynamic_values` supplies finite initial values by unit ID and storage
key. `set_dynamic_value`, `dynamic_energy`, `conditional`, and `repeat` execute
in order against live state. Missing reads, zero divisors, nonfinite results,
noninteger loop counts, and excessive recursion/work fail explicitly. Both
conditional branches are validated, even if only one will execute.

`mechanics.modifier_definitions` declares explicit stack modes, maximum stacks,
optional owner/source event lifetimes, and property contributions. The current
property set is critical damage and damage bonus. Applications retain the
source, snapshot property expressions at application/stack time, and expose
active stacks in unit results. Expiry removes contributions before event
listeners; it does not permanently mutate the supplied base panel. Every
stack mode and lifetime must be supplied: this is not inference of native
Modifier defaults or an implementation of full Modifier callbacks/halos.

`tools/config_runtime.py` extends selected raw lowering with declared runtime
storage scopes, live comparisons, bounded loops, persistent CustomProperty
updates and pure OnStack property contributions. Unknown callbacks, fields,
targets and numeric tasks reject. The caller must still install supported
callbacks at a separately justified event point; compiling a callback fragment
does not install a complete passive. The S2 Anaxa counter fragment has been
executed with synthetic initial counters; the full Anaxa kit, its native AI,
random weakness/bounce rules and full GridFight stage remain incomplete.

The two raw property-name mappings were cross-checked against HSR_Nous
`00185ddc4d57039894caf19467f602e716329e40`'s `adapters/enemy_ability.py`.
Its inferred modifier defaults and unconsumed active-skill callbacks are not
imported. The existing MIT notice is retained in `HSR_NOUS_LICENSE`.

The S2 callback integration also exposes `before_skill_use` at action entry
after payment, and `after_attack` only when the explicit `attack_finish` effect
is executed. The latter represents the selected source's DamagePerformFinish
boundary, not Action completion. `retarget_attack_random` samples one living
member of the current actor's already-hit target list and restores nested target
scope on return; an empty list executes its explicit empty branch.

`elemental_damage` requires complete seven-element panels for every unit. Its
element resistance replaces the legacy resolved scalar; element-specific bonus
adds to common bonus and active property contributions. A separately derived
element RNG stream preserves the existing target/critical stream seeds. Random
selection is the simulator's uniform seven-element contract; client distribution
and RNG are unmeasured. Nested listener hits select their own element context.
Each elemental hit also supplies its `attack_type`. Damage trace events and
`hit_attack_type` conditions retain `pursued` independently of the enclosing
Skill action kind. Hit and death listeners receive that hit's context; nested
hits use their own kind, then restore the caller's context on return. Legacy
damage effects are `direct` and do not inherit a listener's triggering hit kind.

The current raw expression decoder additionally handles multiplication/division
and the selected character's `floor` and `max` calls. Function hashes, argument
counts and stack shape were cross-checked against the S2 pursuit count, parent
weakness-weight clamping, and the GridFight timer. This is a structural inference
from pinned configuration, not a client-interpreter measurement. Unknown functions
and invalid call arities still reject. Numeric S2 callbacks have been executed
together with synthetic skill carriers and checked hit by hit; this does not
complete the parent character kit, its additional skills/weaknesses or a stage.

## Explicit invocation storage

`with_dynamic_scope` creates a fresh ability or task-template frame for one
explicit invocation. `scoped_variable` and `set_scoped_dynamic_value` address
that frame by kind and owning unit, independently of persistent unit values.
An inner frame shadows the entire prior frame of the same kind; missing keys
never fall through to the outer invocation or a same-named persistent value.
Leaving the invocation restores the previous frame, including error returns.
Trace entries retain the scope kind and enter/exit boundaries.

The raw runtime lowering accepts explicit `ContextAbility` and
`ContextTaskTemplate` writes only when they match the supplied storage binding.
It also handles sourced increment operations and definitions with explicit reset
values. An omitted `ResetValue` remains unsupported. Callers still supply the
invocation boundaries and per-source bindings: this is not automatic native
scope discovery, a full Modifier-local store, or native default initialization.
Anaxa's ability `_count` and its SkillTree02 modifier `_count` must be bound
separately; no raw name alone is a sufficient storage location.

## Modifier-owned weakness contributions

`mechanics.natural_weaknesses` provides explicit natural sets by unit ID; an
omitted unit is unknown, while `[]` means a known empty set. When using
`mechanics.modifier_weaknesses`, supply natural sets for every present or future
unit and map each contribution to an existing modifier definition. Active
instances contribute a union of element types. Refreshing or stacking the same
type does not increase the distinct count. Removing or expiring one source's
instance preserves natural weaknesses and other active sources' contributions.

Policy observations and unit results expose `natural`, `attached`, and `all`
sets when the natural input is known. `weakness_count` trace rows observe state
changes; they do not dispatch native OnBeingStackWeakness/OnModifierRemove
callbacks. Attachments do not implicitly change resistance, gate toughness
damage, install reveal marks, or infer cleanup on caster death. Those operations
require their own sourced rules and explicit modifier lifetime policies.

`tools/config_weakness.py` accepts only selected explicit owner-bound
`StackWeakness(OPType=Attach)` leaves. It rejects missing fields, other numeric
tasks and duplicate elements. Anaxa's seven UltraDebuff OnStack leaves have been
executed under artificial installation/cleanup policies. This is not a full
UltraDebuff import: its status-resistance/control tasks, OnPhase1/OnDestroy
callbacks and native lifetime remain outside that leaf selection. The passive
random-weakness template still encounters omitted ResetValue defaults, and the
common weakness modifier reads an unsupplied MDF_PropertyValue; neither becomes
zero. Full per-hit weakness/reveal execution therefore remains incomplete.

## Compatibility and verification

The old absolute `next_av` path is gone. Three intentional baseline changes are:
self-advance must name the next action explicitly, revival preserves distance,
and charged penalty AV cannot exceed the current cycle remainder. Formula/RNG,
resource, CLI, serialization, replay and batch tests remain in place. Lifecycle
tests cover turn-entry payment, insertion boundaries, target retention, trace
truncation, dynamic speed, event expiry, cycle ties and terminal precedence.

Source repository license: GPL-3.0-only. HSR_Nous formula attribution remains in
`src/math.rs`; distance semantics reference f8935ddb407b297732fbd3c624d55897e3e648b9,
with the MIT notice in `HSR_NOUS_LICENSE`. Raw game data is not redistributed here.


## Optional finite queue and settlement

`mechanics.queue_capacity` opts into a finite input queue in `waves[0]` with
capacity 1..32 and up to 3200 unique enemies. Other waves must be absent. Empty
slots refill only at complete-action boundaries before subsequent inserts;
revival holds the slot, and remaining queued enemies prevent premature victory.
The result records active slots and unspawned IDs. Existing wave inputs remain
unchanged when this field is absent.

`mechanics.settlement` requires the named policy, exactly one positive weight per
enemy instance, minimum progress, HP before/cap, failure loss and recovery. All
parameters are emitted; there are no implicit HP defaults. Cancellation and
ActionLimit preserve an unprocessed snapshot. Trial ledgers now carry remaining
AV and settlement, and only completed settlements contribute to HP averages.
See [the full policy contract](ANAXA_SCENARIO.md#finite-queue-and-business-result).

`has_modifier` conditions query self, the bound ability target or any living
enemy without consuming RNG. Unknown modifier IDs and other target selectors
are rejected. This narrowly supports the Anaxa disclosure gate and is not a
native modifier callback interpreter.
