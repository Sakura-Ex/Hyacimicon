"""Check raw Config dynamic identities and observed state-update expressions."""
import unittest
from config_expression import compile_expression, stable_hash


class ExpressionTests(unittest.TestCase):
    """Verify fixed source identities and reject unsupported numeric instructions."""

    def test_named_dynamic_hashes_match_config_declarations(self):
        """Three independently stored raw hashes must map back to their names."""
        for name, key in [('Skill_Count', '-169675107'), ('Purse_Attack_Count', '-372803990'),
                          ('GridFight_S2_Current_MagicLayer', '-1581744091')]:
            self.assertEqual(stable_hash(name), key)

    def test_observed_increment_and_subtraction_remain_runtime_expressions(self):
        """Anaxa's count update must read live state, not its initial value."""
        increment = {'IsDynamic':True, 'PostfixExpr':{'OpCodes':'AQAAAAIR',
                     'FixedValues':[{'Value':1}], 'DynamicHashes':[-169675107]}}
        self.assertEqual(compile_expression(increment, {}), {'op':'add',
            'left':{'op':'variable','key':'-169675107'}, 'right':{'op':'constant','value':1.0}})
        subtract = {'IsDynamic':True, 'PostfixExpr':{'OpCodes':'AQABAQMR',
                    'FixedValues':[], 'DynamicHashes':[-169675107,551330741]}}
        self.assertEqual(compile_expression(subtract, {'551330741':3.0})['right'], {'op':'constant','value':3.0})

    def test_unknown_opcode_and_malformed_stack_reject(self):
        """Unidentified raw functions and missing operands cannot compile."""
        for code in ['AQABAQECBRABEQ==', 'ARE=', 'AhE=']:
            with self.subTest(code=code), self.assertRaises(ValueError):
                compile_expression({'IsDynamic':True,'PostfixExpr':{'OpCodes':code,
                    'FixedValues':[],'DynamicHashes':[1776456860,-1581744091,-1943279159]}}, {})
