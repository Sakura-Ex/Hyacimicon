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
- `mechanics.triggers` binds owner, event, optional subject/action filters, uses
  (zero means unlimited), and effects. Optional event-based duration expires before
  triggering its expiration event. Shields retain their specified normal-turn
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

This is **selected regular-skill-chain execution**, not a complete roster importer.
Provide resistance already resolved for the emitted damage element. The local
acceptance run resolves GridFight Seele Skill01 through its Advanced parent,
reads SkillID 1110201 level 1 and PointID 11102103, and executes it against
MonsterSkillID 801101001. Both use supplied controlled panels. Full selected kit
imports are also attempted and rejected at unsupported passive/skill operations.
These controlled results must not be presented as a real roster win probability.

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
