# Battle input assembly

`run-core` prepares a single `battle_core::model::Scenario` from an effective account and a caller-provided encounter. The account owns precomputed static panel values and resolved equipment. The encounter owns unit instance IDs, actions, starting resources, enemy waves, and combat rules. One `AllyBinding` connects each encounter ally to its effective account character.

1. Obtain a `PlayableAccount` through [account conversion](../account/README.md).
2. Provide the encounter, ally bindings, and an explicit `OpeningPlanarCritDamage` rule to `run_core::assemble_battle`. Include at least one enemy wave. Initial allied HP must fit the account-derived maximum. When using elemental attacks, provide encounter resistance profiles. Assembly fills the ally profiles' elemental damage bonuses from the account.
3. Pass the prepared scenario and a seed to `BattleSession::new`. For diagnostics, serialize the same prepared scenario and load it through `ScenarioFile::from_slice` as described in the [battle module](../battle-simulation/README.md).

## Equipment contract

The 4.7 S2 account snapshot supplies the static set property for planar set `310`. Assembly reads the equipped pair and uses the account panel's final effect resistance to evaluate the inclusive opening threshold. It appends one battle-start listener for a single eligible living wearer. At AV zero the listener grants a persistent critical damage modifier to the allies alive at that point. Later waves retain those modifiers and do not add new instances. If other battle-start listeners are present, those listeners run first in their declared order. [Set effects](../../game-data/relic-set-effects.md) and [account snapshot](../../../core/account/data/s2-v4.7/manifest.json).

Assembly accepts a trial aggregate, a mapped planar group, or a selected owned sphere and rope from the same set. It rejects an encounter with multiple eligible wearers. This slice uses the repository's rule table and a synthetic encounter. Live timing, multiple-wearer behavior, and later eligibility changes remain unverified.

## Run the integration checks

```sh
cargo test -p run-core --locked
```

The tests import and round-trip a repository account fixture, convert the effective panels, and verify the threshold cases. A two-wave fixture compares complete results from the resumable session and from the reloaded diagnostic scenario at the same seed. Its trace checks opening applications and an actual critical hit.
