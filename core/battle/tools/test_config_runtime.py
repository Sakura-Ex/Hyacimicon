"""Artificial control-flow counterexamples; no raw game data is bundled."""
import unittest
from config_actions import Compiler, Unsupported
from config_expression import stable_hash
from config_runtime import RuntimeLowering


class RuntimeTests(unittest.TestCase):
    """Exercise live state lowering and rejection of unimplemented branches."""
    def setUp(self):
        """Prepare artificial immutable parameters and one declared local key."""
        self.compiler = Compiler([], {'7': 2.0}, [], 0.0)
        self.lowering = RuntimeLowering(self.compiler, {stable_hash('count'): 'modifier:count'})

    def test_live_comparison_is_not_folded_with_initial_state(self):
        """Keep a live read on the left and a resolved parameter on the right."""
        node = {'$type':'RPG.GameCore.ByCompareDynamicValue',
                'DynamicKey':{'Value':'count'}, 'CompareType':'GreaterEqual',
                'CompareValue':{'IsDynamic':True,'PostfixExpr':{'OpCodes':'AQAR','FixedValues':[],'DynamicHashes':[7]}}}
        result = self.lowering.condition(node)
        self.assertEqual(result['left'], {'op':'variable','key':'modifier:count'})
        self.assertEqual(result['right'], {'op':'constant','value':2.0})

    def test_unknown_task_in_either_branch_rejects_at_pointer(self):
        """Reject an unsupported operation even in an unselected branch."""
        task = {'$type':'RPG.GameCore.PredicateTaskList',
                'Predicate':{'$type':'RPG.GameCore.ByCurrentSkillType','SkillType':'Skill'},
                'SuccessTaskList':[], 'FailedTaskList':[{'$type':'RPG.GameCore.UnknownNumeric'}]}
        with self.assertRaisesRegex(Unsupported, 'callback/0/FailedTaskList/0: runtime task UnknownNumeric'):
            self.lowering.tasks([task], 'callback')

    def test_storage_scope_and_extra_fields_cannot_be_inferred(self):
        """Reject unknown storage and an unimplemented explicit ContextScope."""
        task = {'$type':'RPG.GameCore.SetDynamicValue','DynamicKey':{'Value':'other'},'Value':{'Value':1.0}}
        with self.assertRaisesRegex(Unsupported, 'undeclared runtime storage'):
            self.lowering.tasks([task], 'callback')
        task['DynamicKey']['Value'] = 'count'
        task['ContextScope'] = 'ContextAbility'
        with self.assertRaisesRegex(Unsupported, 'unhandled fields'):
            self.lowering.tasks([task], 'callback')

    def test_persistent_custom_property_requires_runtime_value(self):
        """Preserve the current persistent value as a required runtime read."""
        task = {'$type':'RPG.GameCore.ModifyGridFightCustomProperty',
                'CustomType':{'EnumIndex':2,'Value':108},'ModifyValue':{'Value':1.0}}
        effect = self.lowering.tasks([task], 'callback')[0]
        self.assertEqual(effect['key'], 'grid_fight_custom:108')
        self.assertEqual(effect['value']['left'], {'op':'variable','key':'grid_fight_custom:108'})

    def test_property_lowering_cannot_skip_lifecycle_callbacks(self):
        """Do not declare a property modifier complete when another callback exists."""
        definition = {'_CallbackList':[{'Event':'OnStack','CallbackConfig':[
            {'$type':'RPG.GameCore.StackProperty','Property':'CriticalDamageBase',
             'TargetType':{'$type':'RPG.GameCore.TargetAlias','Alias':'ModifierOwnerEntity'},
             'PropertyValue':{'Value':0.6}}]}]}
        result = self.lowering.property_changes(definition, 'modifier')
        self.assertEqual(result, [{'property':'crit_damage','value':{'op':'constant','value':0.6}}])
        definition['_CallbackList'].append({'Event':'OnDestroy','CallbackConfig':[]})
        with self.assertRaisesRegex(Unsupported, 'unsupported property modifier callback OnDestroy'):
            self.lowering.property_changes(definition, 'modifier')


if __name__ == '__main__':
    unittest.main()
