"""Counterexamples for strict raw Config lowering; fixtures are artificial."""
import unittest
from config_actions import Compiler, Unsupported


class ConfigTests(unittest.TestCase):
    """Check branch selection, required bindings, and unknown operations."""

    def setUp(self):
        """Create a compiler without bundled copyrighted game data."""
        self.compiler = Compiler([], {'7': 0.2}, ['PointB3'], 10.0)

    def test_inverse_predicate(self):
        """Inverse must invert the selected regular-skill branch."""
        self.assertFalse(self.compiler.predicate({'$type':'RPG.GameCore.BySkillPointActivated','PointTriggerKey':'PointB3','Inverse':True}))

    def test_missing_dynamic_is_not_zero(self):
        """An unresolved runtime binding must stop compilation."""
        with self.assertRaisesRegex(Unsupported, 'unresolved dynamic hash 9'):
            self.compiler.number({'IsDynamic':True,'PostfixExpr':{'OpCodes':'AQAR','FixedValues':[],'DynamicHashes':[9]}})

    def test_observed_negation_opcode(self):
        """The sourced delay expression uses a load followed by negation."""
        self.assertEqual(self.compiler.number({'IsDynamic':True,'PostfixExpr':{'OpCodes':'AQAOEQ==','FixedValues':[],'DynamicHashes':[7]}}),-0.2)

    def test_unknown_task_is_located(self):
        """Never drop an unknown logic node."""
        with self.assertRaisesRegex(Unsupported, 'probe/0: task UnknownDamage'):
            self.compiler.tasks([{'$type':'RPG.GameCore.UnknownDamage'}],'probe')

    def test_hit_energy_is_lowered_with_ratio(self):
        """Hit energy is numerical behavior, separate from split damage."""
        effects=self.compiler.tasks([{'$type':'RPG.GameCore.DamageByAttackProperty','TargetType':{'$type':'RPG.GameCore.TargetAlias','Alias':'AbilityTargetEntity'},'AttackProperty':{'$type':'RPG.GameCore.AttackData','DamageType':{'DamageType':'Quantum'},'DamagePercentage':{'Value':2.5},'SPHitRatio':{'Value':0.5}}}],'probe')
        self.assertEqual(effects[-1],{'type':'energy','amount':5.0,'target':'ability_target'})


if __name__=='__main__':
    unittest.main()
