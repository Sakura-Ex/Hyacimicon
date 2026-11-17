# Evidence needed to align the battle model with the game

The executable example in this module uses explicit development inputs and named
provisional policies. It can test an entire numerical battle and its configured
settlement. It has not yet been matched to a complete recorded Currency Wars
battle. There is currently no such sample available from the project owner.
Missing recordings do not prevent further implementation.

This document describes what to collect when material becomes available. Start
with one well-recorded battle; there is no requirement to collect every character
or fill every field before sharing useful evidence. Unknown values remain unknown.

## First useful package

1. Game/resource version, collection date, season, route/node and difficulty.
2. Visible preparation details: all units, equipment, traits, environment effects,
   initial resources and carried states. Include detail screens not visible in
   the recording. Distinguish an empty equipment slot from an unrecorded slot.
3. Continuous recording from preparation through settlement, including all waves,
   deaths and revivals. Record auto/manual mode, speed setting and any intervention.
4. Result screen, visible progress, remaining battle AV/cycles, and whole-run HP
   before and after settlement, including separate damage/healing notifications.

Provide existing exports/logs if available, with their format and origin. No
account credentials, access tokens or personal identifiers are needed. A player
does not need to recover hidden IDs, annotate every frame, infer the game RNG seed
or calculate hidden stats. Developers map the versioned configuration, extract
events and ask only about the first unresolved discrepancy.

## Complete input checklist

The tables guide developer reconstruction of the chosen battle. Values already
visible in a recording or export need not be transcribed again. Mark evidence as
observed, derived from identified configuration, or unknown, and retain its time
point: preparation, entry before actions, after a particular action, or settlement.

| Area | Information to retain | Why it matters |
| --- | --- | --- |
| Identity | Client/resource version, date, server version if relevant, season/subseason, route, chapter/section, node type/name, stage identifiers when available | Prevents mixing old and new mechanisms or data |
| Difficulty | Rank/division, difficulty modifiers, overclock settings, extra node/enemy difficulty | Resolves actual panels and penalties |
| Scene | Environment, investment strategies, portal/field/node effects, levels/stacks, conditions, durations and their sources | Entry effects and ongoing callbacks can alter every result |
| Battle budget | Initial cycle count, each cycle's AV, special first cycle, current remainder, additions and penalties | Determines the requested remaining total battle AV |
| Run resources | Current whole-run HP, HP cap, effects that change either, battle-caused loss and recovery modifiers | Unit HP cannot substitute for run HP |
| Goal | Initial progress, required progress, weights and denominator, clear/timeout/failure conditions | Separates enemy clearance, threshold qualification and run survival |
| Friendly roster | Front/back positions and order, character/form, instance labels, owned/trial status, star rank, level/ascension, eidolons, skill/talent levels, active traces and mode-specific replacements | A character name or star rank alone does not identify the active kit |
| Effective panels | Max/current HP, attack, defense, speed, crit, break/effect/energy stats, elemental bonuses/resistance, damage/healing/shield modifiers and penetration | Distinguishes supplied effective values from buffs still to apply |
| Equipment | Light cone and upgrades, relic/ornament details if applicable, mode equipment/gems/projections, upgrades and holder, explicit empty slots | Static panels do not encode triggered effects |
| Traits | Active trait IDs/names, tier, count and contributors, plus actual roster/equipment | Enables checking the derived trait set |
| Initial state | Energy/cap, shared skill points/cap, shields and sources, action position, marks, statuses, stacks, durations, source units and cross-battle counters | Prevents invented zero/full-resource defaults |
| Entry and summons | Techniques/entry skills, preselected targets, summons, owner, initial presence and targetability | Distinguishes pre-existing state from entry callbacks |
| Revival | Finite/unlimited policy, HP restored, energy/status retention, AV cost and source, penalties near a cycle deadline | A revival changes both unit state and battle budget |
| Enemy instances | IDs/names/forms, order/position, level/rank, skill replacements, effective HP/attack/defense/speed/toughness/resistance/weakness/immunity and initial state | Defines actual opponents rather than an enemy candidate pool |
| Queue | Initial active enemies, capacity/slots, full pending queue if available, entry/refill timing, next-action distance, wave reset/persistence | A temporarily empty field can still have pending enemies |
| Enemy behavior | Skill rotation, counters, phases, charges, targets, summons, transformations, escape and break/recovery behavior | May alter progression without a simple kill |
| Progress accounting | Per-instance contribution, HP-based contribution, denominator, treatment of summons/escapes/phases, threshold units and comparison | Needed to replace the provisional weighted-kill policy |
| Settlement | Base penalty, progress formula, rounding, recovery formula, loss/recovery order, HP cap/floor and all overrides | Needed to replace the provisional flat-penalty policy |

Keep base/preparation stats and entry-effective stats distinguishable. A supplied
effective panel can simplify input assembly, but equipment/trait callbacks still
need their actual rules. Preserve all effects present in the selected recorded
battle; do not disable inconvenient mechanics to make a replay match.

A recording identifies enemies that appeared. It generally cannot reveal the
unseen queue or its generator. Developers must obtain that separately or retain
an explicitly supplied queue; a pool of possible enemies is not a spawn sequence.
An empty static stage monster list can mean runtime generation, not an empty fight.

## Actions, randomness and comparison output

Record auto/manual settings and any target selection or intervention at a relative
action point. Developers reconstruct actor, skill, primary target and hit targets;
basic fallback when skill points are unavailable; ultimate conditions/order;
extra turns; follow-ups; bounce targeting and tie handling.

For each observable action, retain damage segments/crits, HP/shield/toughness,
energy/skill points, statuses, break, deaths/revivals, AV penalties and action
position changes. Keep occluded/unshown values unknown. Animation duration is not
battle AV; a shared video frame is not proof of engine event order.

Record random targets, bounce choices, elements, weaknesses and status outcomes
separately. A simulator seed reproduces the simulator, not the client RNG. An
observed-choice replay can locate a first discrepancy but must be labelled; never
inject observed damage, HP or the final result and call it a prediction. Identical
seed replays are not independent evidence for an in-game win probability.

Compare the final displayed result, progress, remaining total AV/cycles, whole-run
HP before/loss/recovery/after, final unit states and deaths/revivals. Retain exit,
disconnect or cancellation information. Separate model-fitting samples from later
verification samples. A software regression test verifies its stated policy, not
automatically the native game rule.

## Specific rule evidence that would improve this implementation

| Priority | Needed evidence | Outputs affected |
| --- | --- | --- |
| 1 | Native progress/settlement semantics, field defaults, enemy categories and denominator, threshold, penalty/recovery/rounding and cap order; relevant maintainer explanation or configuration consumer | Progress qualification and final whole-run HP |
| 1 | Full enemy queue or generation inputs, capacity, slot/refill timing, entry action position and state retention | Action order, win/loss, remaining AV |
| 1 | Selected character's complete mode-specific kit and initialization, persistent counters, active replacements and triggers | Damage, energy, follow-up frequency and all final outputs |
| 2 | Evidence for random weakness implant/reveal lifetime, bounce preference, full bonus skill, control, per-weakness defense ignore and live enemy-count effects | Anaxa damage/targeting/action count and enemy turns |
| 2 | Normal versus free skill energy per hit, actual hit count and energy modifiers/cap, including conditional extra bounces | Ultimate timing and remaining AV |
| 2 | Revival cost near less/equal/more remaining cycle AV, retention rules and any carryover | Remaining AV and survival |
| 2 | Normal TurnEnd/status timing, simultaneous deadlines, inserted ultimate/extra-turn/follow-up order | Action order and remaining AV |
| 2 | Native AI inheritance, target scoring, omitted defaults and tie/random/ultimate priority behavior | Agreement with in-game auto battle |
| 2 | The owner-reported ally damage factor 0.3: its actual property group and interaction with other reduction factors | Damage without accidentally applying the same reduction twice |

For Anaxa, useful visible details include cross-battle growth stacks, the full
skill sequence, normal/free/bonus skill distinction, pursuit hit count/elements,
natural/attached weaknesses and disclosure. Developers map internal variables,
local scopes, expression operations and raw configuration defaults.

To distinguish settlement formulas, progressively collect matched observations
around a progress threshold; equal progress with different remaining AV; equal
progress/AV with different starting run HP; and different enemy categories or
residual HP. Retain uncontrolled differences. The first package does not need all
these comparisons or a large sample count.

Maintainers can also supply versioned semantics for the native wave-settlement
operation, progress types/defaults, stage/season binding and omitted level files.
An initial run-HP constant does not establish the cap. A field name alone does not
establish its formula. Fixed simulator policies remain valid explicit inputs
while native automatic behavior is being investigated.

## Storage and handoff

Public code, this request and shareable conclusions belong in
`Sakura-Ex/hsr-currency-wars-ai` / PR #8. Player recordings, screenshots, private
configuration and internal notes belong in the separately authorized private
`Sakura-Ex/hsr-currency-wars-data` repository using its existing storage practice.
Do not copy private raw tables or assets into this public module or PR.

Record a sample label/date, version, data snapshot ID, entry category/ID, private
relative paths and missing sections. Developers add source/data revisions, hashes,
adapter/policy versions, input hash, simulator seed, output and termination status.
Keep revised evidence alongside its prior version with the changed fields noted.
Publish only approved snapshot references and shareable conclusions.

On receipt: check version and recording completeness, map the selected battle's
reachable mechanics to input/evidence, reproduce it, then investigate the first
disagreement. Name the missing information, its effect on results and a practical
way to obtain it; do not repeatedly request values already supplied.
