"""Compile selected raw Config skill chains into battle_v2 effects.

This operates on explicitly selected skills and resolved combat panels. It never
certifies a complete character kit: passive/AI/stage imports are separately
attempted and unsupported tasks raise with their exact source JSON pointer.
"""
from __future__ import annotations

import argparse
import base64
import collections
import hashlib
import json
import math
from pathlib import Path

# These tasks only control camera/model/animation. Their ordering is recorded;
# numeric tasks are never accepted by a name prefix or silently discarded.
PRESENTATION = {
    'LookAt', 'TriggerAnimStateWithMove', 'TriggerAnimState', 'RadialBlurEffect',
    'MoveToTargetPosition', 'VCameraConfigChange', 'StartAim', 'StopAim',
}
WAITS = {'WaitAnimState', 'WaitSecond', 'WaitTimelineFinish'}
MARKERS = {'DamagePerformFinish', 'SkillPerformFinish', 'SkillExecutionStart'}


class Unsupported(ValueError):
    """An encountered numeric operation has no validated lowering."""


class Compiler:
    """Resolve source abilities and lower one regular skill, preserving order.

    Attributes:
        bindings: Explicit dynamic hash values from Config/Excel or caller state.
        points: Explicit active trace-point names.
        evidence: Ordered source pointers and handling decisions.
    """

    def __init__(self, files: list[Path], bindings: dict[str, float], points: list[str], hit_energy: float):
        """Load raw ability definitions; duplicate names are an input error."""
        self.abilities = {}
        self.modifiers = {}
        self.bindings = bindings
        self.points = set(points)
        if not math.isfinite(hit_energy) or hit_energy < 0:
            raise ValueError("hit_energy must be nonnegative and finite")
        self.hit_energy = hit_energy
        self.evidence = []
        self.stack = []
        self.types = collections.Counter()
        for path in files:
            data = json.loads(path.read_text(encoding='utf-8-sig'))
            self.scan(data)
            for i, ability in enumerate(data['AbilityList']):
                name = ability['Name']
                if name in self.abilities:
                    raise ValueError(f'duplicate ability {name}')
                self.abilities[name] = (ability, f'{path.as_posix()}#/AbilityList/{i}')
                for key, modifier in ability.get('Modifiers', {}).items():
                    self.modifiers[(name,key)] = modifier

    def scan(self, value: object) -> None:
        """Count every typed node, including branches not selected for execution."""
        if isinstance(value, dict):
            if '$type' in value:
                self.types[value['$type']] += 1
            for child in value.values():
                self.scan(child)
        elif isinstance(value, list):
            for child in value:
                self.scan(child)

    def number(self, value: dict) -> float:
        """Resolve literal, dynamic load, and observed postfix arithmetic opcodes.

        Raises:
            Unsupported: Missing binding or unrecognized opcode/stack format.
        """
        if 'Value' in value:
            result = value['Value']
        elif value['IsDynamic'] is False:
            result = value['FixedValue']['Value']
        else:
            expr = value['PostfixExpr']
            codes = base64.b64decode(expr['OpCodes'], validate=True)
            if not codes or codes[-1] != 17:
                raise Unsupported('postfix expression lacks terminator')
            stack = []
            index = 0
            while index < len(codes):
                op = codes[index]
                index += 1
                if op in (0, 1):
                    operand = codes[index]
                    index += 1
                    if op == 0:
                        stack.append(expr['FixedValues'][operand]['Value'])
                    else:
                        key = str(expr['DynamicHashes'][operand])
                        if key not in self.bindings:
                            raise Unsupported(f'unresolved dynamic hash {key}')
                        stack.append(self.bindings[key])
                elif op == 14:
                    stack.append(-stack.pop())
                elif op == 17 and index == len(codes):
                    break
                else:
                    raise Unsupported(f'unsupported postfix opcode {op}')
            if len(stack) != 1:
                raise Unsupported('invalid postfix stack')
            result = stack[0]
        if isinstance(result, bool) or not isinstance(result, (float, int)) or not math.isfinite(result):
            raise ValueError('numeric expression must be finite')
        return float(result)

    def predicate(self, node: dict) -> bool:
        """Evaluate only predicates fixed by the declared regular-skill context."""
        kind = node['$type'].removeprefix('RPG.GameCore.')
        if kind == 'BySkillPointActivated':
            result = node['PointTriggerKey'] in self.points
        elif kind == 'ByIsInsertAction':
            result = False
        elif kind == 'ByIsTurnOwnerEntity':
            if node.get('TargetType') != {'$type':'RPG.GameCore.TargetAlias','Alias':'Caster'}:
                raise Unsupported('turn owner predicate on non-caster')
            result = True
        else:
            raise Unsupported(f'predicate {kind}')
        inverse = node.get('Inverse', False)
        if not isinstance(inverse, bool):
            raise ValueError('predicate Inverse must be boolean')
        return not result if inverse else result

    def ability(self, name: str) -> list[dict]:
        """Validate ability fields, then lower supported phases and references.

        Only OnAdd and OnStart execute. Nonempty unhandled callbacks/metadata
        reject at the field pointer; unused modifier definitions are declarations
        and are checked when an AddModifier task actually references them.
        """
        if name in self.stack or len(self.stack) >= 32:
            raise Unsupported(f'cyclic ability reference {name}')
        if name not in self.abilities:
            raise Unsupported(f'missing ability {name}')
        ability, pointer = self.abilities[name]
        for field, value in ability.items():
            where = pointer + '/' + field.replace('~', '~0').replace('/', '~1')
            reason = None
            if field in {'Name', 'Modifiers'}:
                continue
            if field in {'OnAdd', 'OnStart'}:
                if not isinstance(value, list):
                    reason = 'ability phase must be a task list'
            elif field == 'TargetInfo':
                if value != {'TargetType': 'SkillTargetEntityList'}:
                    reason = 'unsupported ability target'
            elif value is None or value == [] or value == {}:
                self.evidence.append({'pointer': where, 'field': field, 'handling': 'empty_unsupported_field'})
            else:
                reason = 'unsupported ability field'
            if reason:
                self.evidence.append({'pointer': where, 'field': field, 'handling': 'rejected'})
                raise Unsupported(f'{where}: {reason} {field}')
        self.stack.append(name)
        try:
            effects = []
            for phase in ['OnAdd', 'OnStart']:
                effects += self.tasks(ability.get(phase, []), pointer + '/' + phase)
            return effects
        finally:
            self.stack.pop()

    def tasks(self, tasks: list[dict], pointer: str) -> list[dict]:
        """Lower ordered tasks; reject unknown logic at its exact source pointer."""
        effects = []
        for i, task in enumerate(tasks):
            where = f'{pointer}/{i}'
            kind = task['$type'].removeprefix('RPG.GameCore.')
            row = {'pointer': where, 'type': task['$type']}
            self.evidence.append(row)
            try:
                if kind in PRESENTATION | WAITS | MARKERS:
                    row['handling'] = 'presentation_only' if kind in PRESENTATION else 'ordered_marker_zero_av'
                elif kind == 'TriggerAbility':
                    row['handling'] = 'resolve_reference'
                    effects += self.ability(task['AbilityName']['Value'])
                elif kind == 'PredicateTaskList':
                    branch = 'SuccessTaskList' if self.predicate(task['Predicate']) else 'FailedTaskList'
                    row['handling'] = branch
                    effects += self.tasks(task.get(branch, []), where + '/' + branch)
                elif kind == 'DamageByAttackProperty':
                    if task['TargetType'] != {'$type': 'RPG.GameCore.TargetAlias', 'Alias': 'AbilityTargetEntity'}:
                        raise Unsupported('non-single explicit ability target')
                    if set(task) - {'$type','TargetType','AttackProperty','SpecialHitSoundEvent','CanTriggerLastKill'}:
                        raise Unsupported('unhandled damage task fields')
                    attack = task['AttackProperty']
                    hit_energy_ratio = self.number(attack['SPHitRatio']) if 'SPHitRatio' in attack else 1.0
                    allowed = {'$type', 'DamageType', 'DamagePercentage', 'StanceValue', 'HitSplitRatio',
                               'SPHitRatio', 'HitAnimation', 'HitEffect', 'HitEffectHeight', 'HitTimeSlowType',
                               'HitTimeSlowIntensity', 'HitPosHeight', 'HitAngleHorizontal', 'HitAngleVertical'}
                    if set(attack) - allowed:
                        raise Unsupported(f'attack fields {sorted(set(attack)-allowed)}')
                    # Config omits split on unsplit hits. This default is an explicit
                    # single-hit convention, recorded in the lowering evidence.
                    split = self.number(attack['HitSplitRatio']) if 'HitSplitRatio' in attack else 1.0
                    ratio = self.number(attack['DamagePercentage']) * split
                    effects.append(dict(type='damage', scale='attack', ratio=ratio, flat=0,
                                        hits=1, can_crit=True, target='ability_target'))
                    if 'StanceValue' in attack:
                        effects.append(dict(type='toughness', amount=self.number(attack['StanceValue']) * split,
                                            target='ability_target'))
                    effects.append(dict(type='energy', amount=self.hit_energy * hit_energy_ratio, target='ability_target'))
                    row.update(handling='ordered_damage_hit', ratio=ratio, hit_split=split, hit_energy=self.hit_energy * hit_energy_ratio,
                               element=attack['DamageType']['DamageType'])
                elif kind == 'ModifyCurrentSkillDelayCost':
                    effects.append(dict(type='advance_next', fraction=-self.number(task['NormalizedValue']), target='self_unit'))
                    row['handling'] = 'turn_end_next_distance'
                elif kind == 'AddModifier':
                    key = task['ModifierName']['Value']
                    scoped = [(owner,key) for owner in reversed(self.stack) if (owner,key) in self.modifiers]
                    if not scoped:
                        raise Unsupported(f'unresolved modifier {key}')
                    modifier = self.modifiers[scoped[0]]
                    # Accept only inspected preshow bookkeeping with no state callback.
                    callbacks = modifier.get('_CallbackList', [])
                    if set(modifier) <= {'ModifierAffectedPreshowConfig', '_CallbackList', 'Stacking', 'DynamicValues'} and all(
                        c['Event'] == 'OnActionEnd' and c['CallbackConfig'] == [{'$type': 'RPG.GameCore.RemoveSelfModifier'}]
                        for c in callbacks
                    ) and 'ModifierAffectedPreshowConfig' in modifier:
                        row['handling'] = 'preshow_only_modifier'
                    else:
                        raise Unsupported(f'numeric modifier {task["ModifierName"]["Value"]}')
                else:
                    raise Unsupported(f'task {kind}')
            except (KeyError, IndexError, Unsupported) as error:
                row['handling'] = 'rejected'
                raise Unsupported(f'{where}: {error}') from error
        return effects


def bindings_from_config(config: dict, skills: dict[str, dict], points: dict[str, dict]) -> dict[str, float]:
    """Resolve Config dynamic hashes through exact selected Excel ParamList rows."""
    result = {}
    for key, field in config['DynamicValues']['Floats'].items():
        read = field['ReadInfo']
        table = skills if read['Type'] == 'SkillParam' else points if read['Type'] == 'SkillTreeParam' else {}
        if read.get('TriggerKey') in table:
            result[key] = table[read['TriggerKey']]['ParamList'][read['Index']]['Value']
    return result


def main() -> None:
    """Compile a request and write effects plus source/operation coverage evidence."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('request', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    request = json.loads(args.request.read_text(encoding='utf-8-sig'))
    files = [Path(p) for p in request['ability_files']]
    compiler = Compiler(files, request['bindings'], request['active_points'], request['hit_energy'])
    result = {'scope': 'selected_regular_skill_chain_only', 'entry': request['entry'],
              'inputs': [{'path': str(p), 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()} for p in files]}
    try:
        result['effects'] = compiler.ability(request['entry'])
        result['status'] = 'compiled'
    except Unsupported as error:
        result.update(status='unsupported', error=str(error))
    result.update(operations=compiler.evidence, all_scanned_types=dict(compiler.types))
    args.output.write_text(json.dumps(result, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    print(json.dumps({'status': result['status'], 'output': str(args.output), 'effects': len(result.get('effects', []))}))
    if result['status'] != 'compiled':
        raise SystemExit(2)


if __name__ == '__main__':
    main()
