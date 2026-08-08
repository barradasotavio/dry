"""
The check that reads a callback's declaration, in every shape a callback comes
in.

Two halves. The first is the check itself: what it refuses, what it lets
through, and the sentence a developer reads when it refuses. The second is
#11's carry-over — `functools.partial`, an object with `__call__` and a builtin
became legal Api entries when the Api stopped requiring a `PyFunction`, were
verified by hand, and had no test until here. They are also the shapes most
likely to defeat introspection, which is why they are tested against this
module rather than against the Api alone.

Nothing here opens a window. tests/test_signature_over_the_bridge.py does that.
"""

import unittest
from functools import partial
from typing import TYPE_CHECKING, Any, Literal, Optional, Union

from dry.signature import mismatch

if TYPE_CHECKING:
    from decimal import Decimal


def refusal(function: object, *arguments: object) -> str | None:
    """
    The message a Call to `function` with these arguments is refused with, or
    `None` if it is let through. The name is always 'call', so the tests read
    as the developer's sentence rather than as plumbing.
    """
    error = mismatch('call', function, arguments)  # pyright: ignore[reportArgumentType]
    return None if error is None else str(error)


class TypesAreChecked(unittest.TestCase):
    def test_a_wrong_type_names_the_callback_the_parameter_and_both_types(self):
        def save_file(path: str) -> None: ...

        error = mismatch('save_file', save_file, (3,))

        self.assertEqual(
            str(error),
            'save_file expects str for path, received number instead.',
        )

    def test_the_refusal_is_a_type_error(self):
        def save_file(path: str) -> None: ...

        self.assertIsInstance(mismatch('save_file', save_file, (3,)), TypeError)

    def test_a_right_type_is_let_through(self):
        def save_file(path: str) -> None: ...

        self.assertIsNone(refusal(save_file, 'notes.txt'))

    def test_every_shape_in_the_bridge_contract_is_recognised(self):
        def take(
            a: None, b: bool, c: int, d: float, e: str, f: list, g: dict
        ) -> None: ...

        self.assertIsNone(refusal(take, None, True, 1, 1.5, 'x', [], {}))

    def test_a_boolean_is_not_an_integer(self):
        def take(count: int) -> None: ...

        self.assertEqual(
            refusal(take, True),
            'call expects int for count, received boolean instead.',
        )

    def test_an_integer_is_not_a_boolean(self):
        def take(flag: bool) -> None: ...

        self.assertEqual(
            refusal(take, 1),
            'call expects bool for flag, received number instead.',
        )

    def test_an_integer_satisfies_a_float(self):
        def take(ratio: float) -> None: ...

        self.assertIsNone(refusal(take, 1))

    def test_a_float_does_not_satisfy_an_integer_and_says_which_number(self):
        def take(count: int) -> None: ...

        self.assertEqual(
            refusal(take, 1.5),
            'call expects int for count, received number (float) instead.',
        )

    def test_a_union_accepts_either_member(self):
        def take(value: int | str) -> None: ...

        self.assertIsNone(refusal(take, 1))
        self.assertIsNone(refusal(take, 'x'))
        self.assertEqual(
            refusal(take, []),
            'call expects int | str for value, received array instead.',
        )

    def test_an_optional_accepts_null(self):
        def take(value: str | None) -> None: ...

        self.assertIsNone(refusal(take, None))
        self.assertIsNone(refusal(take, 'x'))
        self.assertEqual(
            refusal(take, 2),
            'call expects str | None for value, received number instead.',
        )

    def test_the_typing_spellings_of_a_union_work_too(self):
        def take(a: Optional[str], b: Union[int, str]) -> None: ...

        self.assertIsNone(refusal(take, None, 1))
        self.assertIsNotNone(refusal(take, 1, 1))


class NestingIsNotInspected(unittest.TestCase):
    def test_a_parameterised_list_is_checked_as_a_list(self):
        def take(names: list[str]) -> None: ...

        self.assertIsNone(refusal(take, [1, 2, 3]))
        self.assertEqual(
            refusal(take, 'abc'),
            'call expects list[str] for names, received string instead.',
        )

    def test_a_parameterised_dict_is_checked_as_a_dict(self):
        def take(fields: dict[str, int]) -> None: ...

        self.assertIsNone(refusal(take, {'a': 'not an int'}))
        self.assertIsNotNone(refusal(take, []))


class UncheckableAnnotationsLetTheCallThrough(unittest.TestCase):
    def test_an_unannotated_parameter_is_unchecked_not_any(self):
        def take(value) -> None: ...  # pyright: ignore[reportMissingParameterType, reportUnknownParameterType]

        self.assertIsNone(refusal(take, object()))

    def test_a_name_that_only_exists_for_type_checkers_does_not_break_the_call(self):
        # `Decimal` is imported under TYPE_CHECKING at the top of this file, so
        # evaluating this annotation would raise NameError. PEP 649 is what
        # keeps that from happening inside the Bridge.
        def take(amount: 'Decimal') -> None: ...

        self.assertIsNone(refusal(take, 'anything at all'))

    def test_a_forward_reference_to_a_type_defined_later_is_unchecked(self):
        def take(later: 'DefinedBelow') -> None: ...

        class DefinedBelow: ...

        self.assertIsNone(refusal(take, 1))

    def test_any_is_unchecked(self):
        def take(value: Any) -> None: ...

        self.assertIsNone(refusal(take, 1))

    def test_object_is_unchecked(self):
        def take(value: object) -> None: ...

        self.assertIsNone(refusal(take, 1))

    def test_a_developers_own_class_is_not_second_guessed(self):
        class Settings: ...

        def take(settings: Settings) -> None: ...

        self.assertIsNone(refusal(take, {'theme': 'dark'}))

    def test_a_literal_is_not_treated_as_a_type(self):
        def take(mode: Literal['read', 'write']) -> None: ...

        self.assertIsNone(refusal(take, 'anything'))

    def test_a_union_with_one_unreadable_member_is_unchecked_entirely(self):
        def take(value: 'int | Decimal') -> None: ...

        self.assertIsNone(refusal(take, 'a string'))

    def test_a_signature_that_cannot_be_read_at_all_is_unchecked(self):
        class Hostile:
            @property
            def __signature__(self) -> object:
                raise RuntimeError('no.')

            def __call__(self, path: str) -> None: ...

        self.assertIsNone(refusal(Hostile(), 3))


class ArityIsChecked(unittest.TestCase):
    def test_too_few_arguments_name_the_count_and_what_is_missing(self):
        def save_file(path: str, data: str) -> None: ...

        self.assertEqual(
            refusal(save_file, 'notes.txt'),
            'call takes 2 arguments, received 1. data was not passed.',
        )

    def test_too_many_arguments_name_the_count(self):
        def save_file(path: str) -> None: ...

        self.assertEqual(
            refusal(save_file, 'a', 'b'),
            'call takes 1 argument, received 2.',
        )

    def test_arity_is_reported_before_any_type_is(self):
        def save_file(path: str, data: str) -> None: ...

        self.assertIn('was not passed', refusal(save_file, 1) or '')

    def test_a_default_makes_an_argument_optional(self):
        def take(path: str, encoding: str = 'utf-8') -> None: ...

        self.assertIsNone(refusal(take, 'notes.txt'))
        self.assertIsNone(refusal(take, 'notes.txt', 'latin-1'))
        self.assertEqual(
            refusal(take, 'a', 'b', 'c'),
            'call takes at most 2 arguments, received 3.',
        )

    def test_a_default_is_never_type_checked_against_itself(self):
        # The default is the developer's own value, not something that crossed
        # the Bridge, so it is left alone even when it contradicts the
        # annotation.
        def take(path: str, encoding: int = 'not an int') -> None: ...  # pyright: ignore[reportArgumentType]

        self.assertIsNone(refusal(take, 'notes.txt'))

    def test_a_partly_optional_callback_reports_at_least(self):
        def take(path: str, encoding: str = 'utf-8') -> None: ...

        self.assertEqual(
            refusal(take),
            'call takes at least 1 argument, received 0. path was not passed.',
        )

    def test_a_callback_taking_nothing_refuses_an_argument(self):
        def take() -> None: ...

        self.assertEqual(refusal(take, 1), 'call takes 0 arguments, received 1.')


class VariadicAndKeywordParameters(unittest.TestCase):
    def test_star_args_accepts_any_number_of_arguments(self):
        def take(*values: int) -> None: ...

        self.assertIsNone(refusal(take))
        self.assertIsNone(refusal(take, 1, 2, 3))

    def test_star_args_is_checked_one_element_at_a_time(self):
        def take(*values: int) -> None: ...

        self.assertEqual(
            refusal(take, 1, 'two'),
            'call expects int for values, received string instead.',
        )

    def test_star_kwargs_is_ignored_because_no_call_can_fill_it(self):
        def take(path: str, **rest: int) -> None: ...

        self.assertIsNone(refusal(take, 'notes.txt'))

    def test_a_required_keyword_only_parameter_is_refused_with_a_way_out(self):
        def take(path: str, *, mode: str) -> None: ...

        self.assertEqual(
            refusal(take, 'notes.txt'),
            'call requires the keyword-only parameter mode, and a Call passes '
            'positional arguments only, so no Call can reach it. Give mode a '
            'default, or take it positionally.',
        )

    def test_a_keyword_only_parameter_with_a_default_is_fine(self):
        def take(path: str, *, mode: str = 'w') -> None: ...

        self.assertIsNone(refusal(take, 'notes.txt'))

    def test_a_positional_only_parameter_is_checked_like_any_other(self):
        def take(path: str, /) -> None: ...

        self.assertIsNone(refusal(take, 'notes.txt'))
        self.assertIsNotNone(refusal(take, 3))


class TheCallableShapesTheApiAccepts(unittest.TestCase):
    """
    #11 replaced `Py<PyFunction>` with `Py<PyAny>` and a `callable()` guard, so
    these three became legal Api entries. They are checked here in the same
    terms as a plain function.
    """

    def test_a_partial_is_checked_on_the_parameters_it_has_left(self):
        def save_file(path: str, data: str) -> None: ...

        bound = partial(save_file, 'notes.txt')

        self.assertIsNone(refusal(bound, 'contents'))
        self.assertEqual(
            refusal(bound, 3),
            'call expects str for data, received number instead.',
        )
        self.assertEqual(
            refusal(bound, 'a', 'b'),
            'call takes 1 argument, received 2.',
        )

    def test_a_partial_binding_a_keyword_is_checked_on_the_rest(self):
        def save_file(path: str, data: str) -> None: ...

        self.assertIsNone(refusal(partial(save_file, data='x'), 'notes.txt'))

    def test_an_object_with_call_is_checked_without_counting_self(self):
        class Saver:
            def __call__(self, path: str) -> None: ...

        self.assertIsNone(refusal(Saver(), 'notes.txt'))
        self.assertEqual(
            refusal(Saver(), 3),
            'call expects str for path, received number instead.',
        )

    def test_an_object_with_an_async_call_is_checked_the_same_way(self):
        class Saver:
            async def __call__(self, path: str) -> None: ...

        self.assertIsNone(refusal(Saver(), 'notes.txt'))
        self.assertIsNotNone(refusal(Saver(), 3))

    def test_a_bound_method_is_checked_without_counting_self(self):
        class Saver:
            def save(self, path: str) -> None: ...

        self.assertIsNone(refusal(Saver().save, 'notes.txt'))
        self.assertIsNotNone(refusal(Saver().save, 3))

    def test_a_builtin_with_a_readable_signature_is_checked(self):
        # len(obj, /) is one of the builtins Argument Clinic describes.
        self.assertIsNone(refusal(len, [1, 2]))
        self.assertEqual(refusal(len, [], []), 'call takes 1 argument, received 2.')

    def test_a_builtin_with_no_signature_at_all_is_let_through(self):
        # max has no introspectable signature. An Api may still hold it, and a
        # Call to it must not be refused for a declaration nobody can read.
        self.assertIsNone(refusal(max, 1, 2, 3))
        self.assertIsNone(refusal(max))

    def test_a_builtin_method_of_an_object_is_let_through(self):
        self.assertIsNone(refusal('abc'.startswith, 'a'))

    def test_a_builtin_type_used_as_a_callback_is_let_through(self):
        self.assertIsNone(refusal(int, '12'))

    def test_a_lambda_is_checked(self):
        self.assertEqual(
            refusal(lambda a, b: None, 1),
            'call takes 2 arguments, received 1. b was not passed.',
        )


if __name__ == '__main__':
    unittest.main()
