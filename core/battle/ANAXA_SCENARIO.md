# Runnable Anaxa S2 scenario

`examples/anaxa-s2-full-battle.json` runs one battle from initial state to combat
termination and configured whole-run HP settlement. It uses the same engine for
single runs and batches. This is a complete execution path under the declared
approximation policies, not a complete or client-accurate reproduction of Anaxa.
No actual player battle has been used for end-to-end numerical validation.

## Run and change inputs

From the repository root:

```text
python core/battle/tools/build_anaxa_full_battle.py core/battle/examples/anaxa-s2-full-battle.json
cargo run -p battle-core --release --locked -- validate core/battle/examples/anaxa-s2-full-battle.json
cargo run -p battle-core --release --locked -- simulate core/battle/examples/anaxa-s2-full-battle.json 42
cargo run -p battle-core --release --locked -- batch core/battle/examples/anaxa-s2-full-battle.json 8 42
```

The builder needs only Python's standard library and has no private-data inputs.
The checked-in JSON is runnable without Python. To replay a batch trial, pass its
ledger `seed` to `simulate` with the unchanged JSON (the master seed is different).
The fingerprint includes the panels, queue, policy parameters and assumptions.

The scene selects one Anaxa S2 role `140502`, basic level 6 and skill/ultimate
level 10, without equipment, active trait bonuses, eidolons or technique. All
effective panels are **developer-selected inputs**, not catalog-derived player
builds. HP 6000, ATK 600, SPD 120 and zero initial energy are explicit. Initial
S2 magic layers and counters are zero. Basic/skill selection follows legal
resource constraints rather than a scripted action list. This does not model
account-to-effective-build conversion or arbitrary equipment/roster composition.

Six distinct enemy instances each have developer HP 2500, ATK 30 and SPD 80.
They use the selected public `Monster_AML_Minion01_00` Skill01 chain: one
2.5 ATK Quantum hit, then an explicitly configured 10 energy to the living
ability target. Other enemy skills/passives are excluded. This is not a claim
that this exact queue, panel or energy parameter occurs at a native node.

## Implemented path and approximation ledger

| Part | Behavior in this scenario | Accuracy boundary |
| --- | --- | --- |
| Fixed strategy | Legal ultimate inserts; normal skill when affordable, otherwise basic; primary target lowest HP fraction, stable entry-index tie | Declared action ordering, not native ComplexAI |
| Basic | 1.0 ATK Wind hit, one SP, 30 energy | Includes the selected trace-1 energy allowance |
| Skill | One selected and four uniform random living-enemy Wind hits, each 0.7 ATK; one SP; 30 total energy | No native prefer-unhit targeting or opcode 11/13 interpretation; total energy explicitly chosen |
| Ultimate | Costs 140 before inserted TurnStart; attaches seven weaknesses/disclosure, then 1.6 ATK Wind AoE; gains 5 | No hard control or control resistance |
| Disclosure | Expires at the affected enemy's next TurnStart; basic/skill on disclosed entry target queues one free 0.7 ATK segment | HSR_Nous conservative single-segment approximation; no complete free five-hit skill or its energy/toughness |
| Free segment | Runs through existing FollowUp insertion, cannot recurse; reselects primary at its entry | Native Skill classification/target retention not reproduced; explicit S2 hooks count it as skill |
| Damage bonus | Disclosed entry target gives +0.3 damage to this whole action, including bounces/pursuit; ultimate applies before AoE | Native per-target attribution is approximated |
| S2 | Every two paid/free skills adds a magic layer; after skill attacks, floor(layers/5) pursuits choose living previously hit targets, 0.7 ATK random element; +0.6 critical damage and +0.3 common damage | Existing source-derived callbacks; seven-element uniform RNG remains a simulator policy |
| Turn energy | +30 at TurnStart when no living enemy has disclosure | Includes inserted ultimate TurnStart under current loop semantics; needs native alignment |
| Damage/toughness | Existing formula and explicit 0.3 ally factor, seven-element panels and unbroken toughness factor | No inferred extra difficulty factor; no toughness damage or break recovery |

Omitted mechanics include per-hit random weakness implantation, weakness-count
reveal outside ultimate, per-weakness defense ignore, enemy-count skill bonus,
static trace-2 bonuses, other hit-taken energy, native AI and full modifier/control
semantics. These omissions can materially change damage, order, victory and run
HP. They are also emitted in `assumptions`; an omission is not represented as a
successfully imported raw task. Raw compilers keep rejecting unknown operations.

## Finite queue and business result

`mechanics.queue_capacity` opts into `finite_queue_after_action_v1`: exactly one
`waves[0]` is the complete ordered input queue (1..3200 distinct instances), with
capacity 1..32. Without this field, existing static-wave behavior is unchanged.
No enemy generation, escape, summoning or phase transition is implied.

Initial entries fill ascending slots. At complete-action boundaries, dead slots
are filled in ascending order, before subsequent inserts, with full action
distance and explicit initial values. No replacement occurs inside a multi-hit
action. Immediate revival retains its slot and is not defeated progress. A
pending queue prevents empty-board victory. New entries do not dispatch global
BattleStart or WaveStart; their existing unit-event hooks become eligible.
`queue` reports policy, active slots and unspawned IDs; `units` contains all units
that have entered, including defeated ones. Queue inspection is result metadata,
not extra hidden strategy observation.

`outcome` remains the combat result: complete enemy elimination wins; living
enemies/queued instances at AV exhaustion lose. Elimination at a completed-action
boundary takes priority over expiry; same-time action/cycle ordering uses the
existing explicit `unit_before_cycle` flag (false here). Threshold progress never
ends the battle early. `minimum_progress_met` and `run_alive` are separate fields:
a combat defeat can meet minimum progress and the run can continue.

`mechanics.settlement` explicitly selects `weighted_kills_flat_penalty_v1`:

1. Sum the positive input weights of final-dead enemy instances once. Divide by
   the fixed sum of all queue/wave instance weights; initial revival deaths do not
   contribute. Minimum progress uses `>=` the input threshold.
2. If the threshold is unmet, request `failure_loss`; otherwise request zero.
   Apply `min(requested_loss, hp_before)`.
3. Apply `min(recovery, hp_cap - hp_after_loss)`; no rounding or cap changes.
   This named provisional order permits recovery after loss reaches zero.
4. Emit requested/actual loss and recovery, clipping, before/cap parameters,
   `hp_after` and `run_alive = hp_after > 0`.

The example uses six equal weights, threshold 0.5, HP before 60, cap 100, failure
loss 20 and recovery 3, all developer inputs. `run_hp_loss` is actual loss before
recovery, not net HP change. Legacy inputs without a settlement policy retain
unknown run HP. Cancellation and ActionLimit have Incomplete outcome and
`not_applied_incomplete` settlement with null after/loss/recovery; runtime errors
remain errors. Batches exclude Incomplete trials from victory probabilities and
aggregate HP only for `settled_count` trials. Trial ledgers include remaining AV
and full settlement parameters/results.

## Sources and validation scope

- HSR_Nous, MIT, commit `f8935ddb407b297732fbd3c624d55897e3e648b9`:
  `data/library/characters/1405_那刻夏.yaml` and
  `tests/test_anaxa_template_e2e.py`. The action/bonus/energy/disclosure structure
  is adapted; the Python engine and generic YAML interpreter are not copied.
  Its own unresolved behavior remains unresolved here. The retained
  [MIT notice](HSR_NOUS_LICENSE) applies to this adapted reference work.
- Public DimbreathBot/TurnBasedGameData commit
  `dad9155eb00fdeda31fb8456507c53590d49071f`: existing selected Anaxa parent/S2
  ability and common modifier evidence; `Monster_AML_Minion01_00_Ability.json`
  Skill01 chain and `MonsterSkillConfig.json` bindings. Source-derived values
  are described above; raw dumps and private catalog tables are not bundled.
- The newer partial HSR_Nous adapter snapshot is not substituted for the pinned
  character template. The private companion catalog is not a public input.

Tests cover a natural scene with all declared action types and S2 pursuit,
hand-calculated queue replacement/progress/HP, revival, deadline ties,
cancellation, trace independence and same-seed single/batch replay. These are
software/declared-policy checks, not evidence of matching native game outcomes.
See [data requirements](DATA_REQUIREMENTS.md) for materials needed to align the
model when recordings or configuration evidence become available.

For the checked-in scene, seed 42 clears all six enemies at 1250 elapsed AV,
with approximately 1150 remaining AV and run HP 60 -> 63 (loss 0, recovery 3).
Its 40 actions include 6 basics, 9 paid skills, 7 ultimates, 4 bonus segments and
14 enemy attacks; S2 adds 3 pursued hits inside those actions. No initial energy
or magic layers were injected. With only the AV budget/cycle changed to 700,
seed 42 instead defeats two of six enemies before timing out, misses the 0.5
threshold and settles HP 60 - 20 + 3 = 43 while the run remains alive.
These are declared-model outputs, not measured game performance.
