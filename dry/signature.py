"""
What a Call carries, checked against what its callback declared.

A Call arrives as JSON and lands on a Python callable the developer wrote. When
that callable declares `def save_file(path: str)` and the frontend passes a
number, the developer today learns about it from whatever their own body raises
several frames later — or from nothing at all, if the body happens to accept
it. This module reads the declaration and says so first: `save_file expects str
for path, received number instead.`

Reading a declaration at runtime is what the Python 3.14 floor bought, and
ADR-0003 names this as the reason. Under PEP 649 an annotation is not evaluated
until something asks for it, and `annotationlib.Format.FORWARDREF` lets this
module ask without accepting the risk that comes with the answer: a name that
only exists under `TYPE_CHECKING`, or one defined later in the file, comes back
as a `ForwardRef` instead of raising `NameError` from inside the Bridge. That is
the whole reason `typing.get_type_hints` is not used here.

The check is deliberately shallow, and deliberately timid.

Shallow: arity and the top level of each argument, and no more. `list[int]`
asks whether an array arrived, not what is in it. Turning a dictionary into a
dataclass is a validation framework, which is not what this library is; the
`default=` hook is where a project that wants one puts it.

Timid: an annotation this module cannot resolve, or can resolve but cannot
express in the JSON data model the Bridge contract is made of, leaves its
parameter unchecked. A Call that runs when it should not have is a bug in the
developer's own code, which they can see and fix. A Call refused because Dry
misread a declaration is a bug in Dry, in code the developer cannot reach. The
second is far worse than the first, so every uncertainty here resolves towards
letting the Call through.
"""

from collections.abc import Callable
from inspect import Parameter, Signature, signature
from logging import getLogger
from types import NoneType
from typing import Any, Union, get_args, get_origin

from annotationlib import Format

_LOGGER = getLogger('dry.bridge')

# What a value crossing the Bridge can be. The JSON data model of ADR-0002,
# split one step finer than JSON itself: a JSON number reaches Python as an int
# or a float, and the difference is exactly what a callback declaring `int` is
# asking about.
_NULL = 'null'
_BOOLEAN = 'boolean'
_INTEGER = 'integer'
_FLOAT = 'float'
_STRING = 'string'
_ARRAY = 'array'
_OBJECT = 'object'

# How each of those is named back to the developer. JSON's own vocabulary,
# because the frontend wrote JSON: an int and a float both arrived as `number`.
_REPORTED = {
    _NULL: 'null',
    _BOOLEAN: 'boolean',
    _INTEGER: 'number',
    _FLOAT: 'number',
    _STRING: 'string',
    _ARRAY: 'array',
    _OBJECT: 'object',
}

# The annotations this module is willing to check, and what each one accepts.
#
# `float` accepts an integer as well as a float, because JSON has one number
# type and JavaScript has one number type: `1` reaches Python as an int however
# the frontend meant it, and refusing it for a parameter declared `float` would
# be the false rejection this module exists to avoid. `int` does not return the
# favour — a float only arrives when the number genuinely had a fraction, since
# `JSON.stringify` writes an integral double without one.
#
# `bool` stands apart from both, against `issubclass(bool, int)`, because a
# callback declaring `int` and receiving `true` has found a real mistake.
_ACCEPTED: dict[object, frozenset[str]] = {
    NoneType: frozenset({_NULL}),
    None: frozenset({_NULL}),
    bool: frozenset({_BOOLEAN}),
    int: frozenset({_INTEGER}),
    float: frozenset({_INTEGER, _FLOAT}),
    str: frozenset({_STRING}),
    list: frozenset({_ARRAY}),
    dict: frozenset({_OBJECT}),
}

# The parameters a Call could ever fill. A Call carries positional arguments
# only — `dry.api.save_file(path)` in JavaScript has nowhere to put a keyword —
# so these are the two kinds that can receive one.
_POSITIONAL = (Parameter.POSITIONAL_ONLY, Parameter.POSITIONAL_OR_KEYWORD)


def mismatch(
    name: str,
    function: Callable[..., object],
    arguments: tuple[object, ...],
) -> TypeError | None:
    """
    The refusal a Call has earned, or `None` to let it run.

    Returns the exception rather than raising it, so that the portal can hand
    it straight to the Completion the Call arrived with: the frontend's Promise
    rejects with a `TypeError` naming the callback, the parameter, what was
    declared and what turned up, which is the same route any other failed Call
    takes back.

    Nothing in here raises. A callable this module cannot read is a callable it
    has no opinion about.
    """
    try:
        declared = _declaration(function)
        if declared is None:
            return None
        return _arity(name, declared, arguments) or _types(name, declared, arguments)
    except Exception:
        _LOGGER.debug(
            "The Call to '%s' could not be checked against its callback, so it "
            'was let through unchecked.',
            name,
            exc_info=True,
        )
        return None


def _declaration(function: Callable[..., object]) -> Signature | None:
    """
    What the callable says about itself, or `None` when it says nothing.

    `FORWARDREF` is the format that makes this survivable: an annotation whose
    name cannot be resolved comes back as a `ForwardRef` rather than raising,
    and `_accepted` treats it as the unknown it is.

    A builtin is the common `None` here. Many carry no signature at all —
    `max` and `str.startswith` are two — and #11 opened the Api to exactly
    those, so they have to pass through unchecked rather than be refused.
    """
    try:
        return signature(function, annotation_format=Format.FORWARDREF)
    except (TypeError, ValueError):
        return None


def _arity(
    name: str,
    declared: Signature,
    arguments: tuple[object, ...],
) -> TypeError | None:
    """
    Whether this many arguments could reach this callable at all.

    A required keyword-only parameter is reported wherever it appears, however
    many arguments arrived, because a Call has no way to fill one: the callback
    can never succeed, and saying so on the first Call beats saying it on every
    one.
    """
    parameters = list(declared.parameters.values())

    unreachable = next(
        (
            parameter
            for parameter in parameters
            if parameter.kind is Parameter.KEYWORD_ONLY
            and parameter.default is Parameter.empty
        ),
        None,
    )
    if unreachable is not None:
        return TypeError(
            f'{name} requires the keyword-only parameter {unreachable.name}, and a '
            f'Call passes positional arguments only, so no Call can reach it. Give '
            f'{unreachable.name} a default, or take it positionally.'
        )

    positional = [
        parameter for parameter in parameters if parameter.kind in _POSITIONAL
    ]
    required = [
        parameter for parameter in positional if parameter.default is Parameter.empty
    ]
    variadic = any(
        parameter.kind is Parameter.VAR_POSITIONAL for parameter in parameters
    )
    given = len(arguments)

    if given < len(required):
        limit = (
            'takes'
            if len(required) == len(positional) and not variadic
            else 'takes at least'
        )
        return TypeError(
            f'{name} {limit} {_count(len(required))}, received {given}. '
            f'{required[given].name} was not passed.'
        )

    if not variadic and given > len(positional):
        limit = 'takes' if len(required) == len(positional) else 'takes at most'
        return TypeError(f'{name} {limit} {_count(len(positional))}, received {given}.')

    return None


def _types(
    name: str,
    declared: Signature,
    arguments: tuple[object, ...],
) -> TypeError | None:
    """
    Whether what arrived is what each parameter said it wanted.

    Only the arguments a Call actually passed are looked at, so a parameter
    left to its default is never questioned about a value the developer chose
    themselves. An annotation on `*args` describes one element, so it is
    checked against each of them.
    """
    bound = declared.bind(*arguments)

    for parameter_name, value in bound.arguments.items():
        parameter = declared.parameters[parameter_name]
        accepted = _accepted(parameter.annotation)
        if accepted is None:
            continue

        values = value if parameter.kind is Parameter.VAR_POSITIONAL else (value,)
        for one in values:  # pyright: ignore[reportUnknownVariableType]
            arrived = _classify(one)
            if arrived is None or arrived in accepted:
                continue
            return TypeError(
                f'{name} expects {_declared_as(parameter.annotation)} for '
                f'{parameter_name}, received {_arrived_as(one, arrived, accepted)} '
                f'instead.'
            )

    return None


def _accepted(annotation: object) -> frozenset[str] | None:
    """
    What a declaration admits, or `None` when this module will not judge it.

    `None` is the answer for everything outside the closed set above, which is
    most of what an annotation can be: a `ForwardRef` left over from a name
    that could not be resolved, a string annotation from a codebase that still
    quotes them, `Any`, `object`, a `Protocol`, a `TypeVar`, a `Literal`, a
    dataclass. A dataclass is the interesting one — a JSON object could never
    be an instance of it, so refusing would even be correct — but a developer
    who annotates a parameter with the shape they are about to build out of it
    has written something this library has no business overruling.
    """
    if annotation is Parameter.empty or annotation is Any or annotation is object:
        return None

    origin = get_origin(annotation)

    # A union is only as checkable as its least checkable member: one
    # `ForwardRef` in `int | Decimal` and the whole parameter goes unchecked,
    # because the value that arrived might well have been the member nobody
    # could read.
    if origin is Union:
        admitted: set[str] = set()
        for member in get_args(annotation):
            accepted = _accepted(member)
            if accepted is None:
                return None
            admitted |= accepted
        return frozenset(admitted)

    # `list[int]` and `dict[str, int]` are checked as `list` and `dict`. The
    # parameters are what the top level deliberately does not look at.
    if origin is not None:
        return _ACCEPTED.get(origin)

    try:
        return _ACCEPTED.get(annotation)
    except TypeError:
        # An unhashable annotation. Nothing to look up, nothing to say.
        return None


def _classify(value: object) -> str | None:
    """
    Which of the Bridge contract's shapes a value turned up as. `bool` is asked
    before `int`, because in Python every `True` is also a `1`.
    """
    if value is None:
        return _NULL
    if isinstance(value, bool):
        return _BOOLEAN
    if isinstance(value, int):
        return _INTEGER
    if isinstance(value, float):
        return _FLOAT
    if isinstance(value, str):
        return _STRING
    if isinstance(value, list):
        return _ARRAY
    if isinstance(value, dict):
        return _OBJECT
    return None


def _declared_as(annotation: object) -> str:
    """
    A declaration as the developer wrote it. `str`, not `<class 'str'>`.
    """
    if annotation is NoneType:
        return 'None'
    if isinstance(annotation, type):
        return annotation.__name__
    return str(annotation).replace('typing.', '')


def _arrived_as(value: object, arrived: str, accepted: frozenset[str]) -> str:
    """
    What turned up, in JSON's words — the frontend's words, since the frontend
    wrote it.

    A number is the one case where JSON's vocabulary is too coarse to explain
    the refusal: a parameter declared `int` that is handed `1.5` would read
    'expects int, received number', which answers nothing. When the declaration
    admits some number and this number is the wrong one, Python's name for it
    is added.
    """
    reported = _REPORTED[arrived]
    collides = any(_REPORTED[shape] == reported for shape in accepted)
    return f'{reported} ({type(value).__name__})' if collides else reported


def _count(number: int) -> str:
    return f'{number} argument' if number == 1 else f'{number} arguments'
