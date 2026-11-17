"""Translate observed raw numeric expressions to the battle runtime expression IR.

Dynamic variable names use the signed stable hash verified against raw Config
declarations. Unknown bytecode operations reject; the IR's general arithmetic
support does not imply every raw opcode has been identified.
"""
import base64
import math


def stable_hash(name: str) -> str:
    """Return the signed 32-bit two-accumulator hash of UTF-16 code units."""
    raw = name.encode('utf-16-le')
    accumulators = [5381, 5381]
    for index in range(0, len(raw), 2):
        code = int.from_bytes(raw[index:index + 2], 'little')
        if code == 0:
            break
        slot = (index // 2) % 2
        accumulators[slot] = ((accumulators[slot] * 33) ^ code) & 0xffffffff
    result = (accumulators[0] + accumulators[1] * 1566083941) & 0xffffffff
    return str(result if result < 2**31 else result - 2**32)


def constant(value: object) -> dict:
    """Build a finite numeric constant, rejecting booleans and missing values."""
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise ValueError('expression constant must be finite and numeric')
    return {'op': 'constant', 'value': float(value)}


def compile_expression(value: dict, bindings: dict[str, float]) -> dict:
    """Preserve unresolved dynamic loads for runtime lookup, never replace by zero.

    Args:
        value: Literal or raw PostfixExpr numeric configuration.
        bindings: Explicit immutable parameters resolved from Config/Excel rows.

    Returns:
        A typed numeric expression consumed by battle_core::expression.

    Raises:
        ValueError: Malformed bytecode, unknown operation, or nonfinite literal.
    """
    if 'Value' in value:
        return constant(value['Value'])
    if value.get('IsDynamic') is False:
        return constant(value['FixedValue']['Value'])
    if value.get('IsDynamic') is not True:
        raise ValueError('expression must declare IsDynamic')
    expr = value['PostfixExpr']
    code = base64.b64decode(expr['OpCodes'], validate=True)
    stack = []
    index = 0
    while index < len(code):
        op = code[index]
        index += 1
        if op in (0, 1):
            if index == len(code):
                raise ValueError('missing postfix operand')
            operand = code[index]
            index += 1
            try:
                if op == 0:
                    item = constant(expr['FixedValues'][operand]['Value'])
                else:
                    key = str(expr['DynamicHashes'][operand])
                    item = constant(bindings[key]) if key in bindings else {'op': 'variable', 'key': key}
            except IndexError as error:
                raise ValueError('postfix operand index out of range') from error
            stack.append(item)
        elif op in (2, 3):
            if len(stack) < 2:
                raise ValueError('postfix binary stack underflow')
            right, left = stack.pop(), stack.pop()
            stack.append({'op': 'add' if op == 2 else 'subtract', 'left': left, 'right': right})
        elif op == 14:
            if not stack:
                raise ValueError('postfix unary stack underflow')
            stack.append({'op': 'negate', 'value': stack.pop()})
        elif op == 17:
            if index != len(code) or len(stack) != 1:
                raise ValueError('invalid postfix termination')
            return stack[0]
        else:
            raise ValueError(f'unsupported postfix opcode {op}')
    raise ValueError('postfix expression lacks terminator')
