"""The gateway must reserve spend before I/O and retain uncertain charges."""
import concurrent.futures
from decimal import Decimal
import unittest

from budget import Budget, MAX_BYTES, OUTPUT_TOKENS


def money(value):
    return Decimal(str(value))


class BudgetTests(unittest.TestCase):
    def test_reserves_worst_case_before_spending(self):
        budget = Budget(limit="0.50", max_calls=80)
        budget.reserve(MAX_BYTES)
        state = budget.snapshot()
        expected = (Decimal(MAX_BYTES + 4096) * Decimal("0.5")
                    + Decimal(OUTPUT_TOKENS) * Decimal("2")) / 1_000_000
        self.assertEqual(state["calls"], 1)
        self.assertEqual(money(state["spent_usd"]), 0)
        self.assertEqual(money(state["reserved_usd"]), expected)

    def test_known_cost_releases_only_the_unused_reservation(self):
        budget = Budget()
        ticket = budget.reserve(1000)
        budget.settle(ticket, 0.0001)
        state = budget.snapshot()
        self.assertEqual(money(state["spent_usd"]), Decimal("0.0001"))
        self.assertEqual(money(state["reserved_usd"]), 0)
        self.assertEqual(state["calls"], 1)

    def test_uncertain_cost_never_releases_reserved_credit(self):
        budget = Budget()
        ticket = budget.reserve(1000)
        reserved = money(budget.snapshot()["reserved_usd"])
        budget.settle(ticket, None)
        self.assertEqual(money(budget.snapshot()["reserved_usd"]), reserved)
        with self.assertRaises(ValueError):
            budget.settle(ticket, 0)
        self.assertEqual(money(budget.snapshot()["reserved_usd"]), reserved)

    def test_invalid_charges_preserve_the_reservation(self):
        for cost in [-1, float("nan"), float("inf"), float("-inf")]:
            with self.subTest(cost=cost):
                budget = Budget()
                ticket = budget.reserve(1000)
                reserved = money(budget.snapshot()["reserved_usd"])
                with self.assertRaises(ValueError):
                    budget.settle(ticket, cost)
                self.assertEqual(money(budget.snapshot()["reserved_usd"]), reserved)
                self.assertEqual(money(budget.snapshot()["spent_usd"]), 0)

    def test_unknown_or_repeated_tickets_cannot_free_credit(self):
        budget = Budget()
        ticket = budget.reserve(1000)
        with self.assertRaises(ValueError):
            budget.settle(ticket + 1, 0)
        budget.settle(ticket, 0)
        with self.assertRaises(ValueError):
            budget.settle(ticket, 0)
        self.assertEqual(money(budget.snapshot()["reserved_usd"]), 0)

    def test_unexpected_high_charge_is_counted_and_blocks_future_calls(self):
        budget = Budget(limit="0.01")
        ticket = budget.reserve(0)
        budget.settle(ticket, "0.02")
        self.assertEqual(money(budget.snapshot()["spent_usd"]), Decimal("0.02"))
        self.assertEqual(money(budget.snapshot()["reserved_usd"]), 0)
        with self.assertRaises(ValueError):
            budget.reserve(0)

    def test_body_and_call_limits_fail_before_reserving_more_credit(self):
        budget = Budget(max_calls=1)
        with self.assertRaises(ValueError):
            budget.reserve(MAX_BYTES + 1)
        self.assertEqual(budget.snapshot()["calls"], 0)
        budget.reserve(0)
        with self.assertRaises(ValueError):
            budget.reserve(0)
        self.assertEqual(budget.snapshot()["calls"], 1)

    def test_spend_cap_blocks_calls_even_when_prior_calls_cost_zero(self):
        budget = Budget(limit="0.001")
        with self.assertRaises(ValueError):
            budget.reserve(0)
        self.assertEqual(budget.snapshot()["calls"], 0)

    def test_parallel_reservations_cannot_oversubscribe_the_cap(self):
        budget = Budget(limit="0.01", max_calls=80)
        def reserve(_):
            try:
                return budget.reserve(0)
            except ValueError:
                return None
        with concurrent.futures.ThreadPoolExecutor(max_workers=20) as pool:
            tickets = list(pool.map(reserve, range(80)))
        state = budget.snapshot()
        self.assertEqual(state["calls"], sum(ticket is not None for ticket in tickets))
        self.assertLessEqual(money(state["reserved_usd"]), money(state["limit_usd"]))
        self.assertGreater(state["calls"], 0)

    def test_negative_request_size_is_rejected_without_increasing_allowance(self):
        budget = Budget()
        with self.assertRaises(ValueError):
            budget.reserve(-1)
        self.assertEqual(budget.snapshot()["calls"], 0)

    def test_invalid_budget_limits_are_rejected(self):
        for limit in ["-1", "NaN", "Infinity", "-Infinity"]:
            with self.subTest(limit=limit), self.assertRaises(ValueError):
                Budget(limit=limit)


if __name__ == "__main__":
    unittest.main()
