# Battle v2

This module executes explicit battle operations with remaining-distance scheduling,
Turn/Action boundaries, event listeners, insertion requests and per-cycle budgets.
It continues PR8's damage/heal/shield math, target/critical random streams, CLI and
batch implementation. It replaces the old `basic_v1` scheduler; old inputs must
select `battle_v2` and account for the changed rules described below.

## Run

From this directory, with the pinned Rust toolchain:

```text
cargo test --locked
cargo build --release --locked
cargo run --release --locked -- simulate examples/team-waves.json 7
```

These public
examples are artificial operation tests. Game data is supplied separately, never
bundled in this module. `validate FILE`, `simulate FILE SEED`, and
`batch FILE TRIALS SEED` retain their CLI argument structure. Seeds remain decimal
strings in results. The library entry points are `validate`, `simulate`,
`simulate_cancellable`, and `simulate_with_policy`.

Results include `provenance`, outcome/termination, elapsed AV, charged penalty AV,
current cycle and remaining cycle/total AV, final unit HP/shield/energy/distance/
speed/toughness, and optional ordered replay events. `Incomplete` denotes a safety
or cancellation stop, not a defeat. Run HP settlement is not part of this module.

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
- `ability_target` chooses the lowest-HP enemy once at Action entry. Every hit
  retains that identity; later hits stop if it dies. Existing `enemy_lowest_hp` and
  `enemy_random_taunt` explicitly reselect per hit. A full game policy's target
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
python -m unittest discover -s tools -p test_config_actions.py
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
