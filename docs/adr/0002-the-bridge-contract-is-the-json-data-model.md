# The Bridge contract is the JSON data model

Values crossing the Bridge follow `json.dumps` / `json.loads` semantics exactly, and anything outside that set raises instead of converting. This replaces a best-effort scheme that guessed at the closest match and produced silent corruption: booleans arrived in JavaScript as numbers, float dictionary keys crashed at serialization time, and `bytes` was documented as `number[]` while having no representation in the model at all.

The rule is chosen for the sentence it fits into rather than the table it generates — "whatever `json` accepts" is already in every Python developer's head, and the `default=` hook they reach for is the same one `json.dumps` gives them.

## Consequences

`set` and `bytes` leave the contract deliberately: neither has a JSON analogue, and both round-trip destructively. Integers outside ±2⁵³ raise rather than silently losing precision. Dictionary keys are coerced to strings exactly as `json.dumps` coerces them, so a round trip returns string keys. `datetime`, `Decimal`, `Enum` and dataclasses are the developer's job, through the hook.
