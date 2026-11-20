"""Reserve a conservative request cost before any paid network I/O."""
from decimal import Decimal, InvalidOperation
from threading import Lock

MODEL = "deepseek/deepseek-v4.1-flash-1017"
OUTPUT_TOKENS = 768
MAX_BYTES = 24000
INPUT_PRICE = Decimal("0.5")
OUTPUT_PRICE = Decimal("2")


class Budget:
    def __init__(self, limit="0.50", max_calls=80):
        self.limit = Decimal(str(limit))
        if not self.limit.is_finite() or self.limit <= 0 or max_calls <= 0:
            raise ValueError("budget and request limit must be positive")
        self.max_calls = max_calls
        self.calls = 0
        self.spent = Decimal(0)
        self.reservations = {}
        self.uncertain = Decimal(0)
        self.lock = Lock()

    def reserve(self, body_bytes):
        if not isinstance(body_bytes, int) or not 0 <= body_bytes <= MAX_BYTES:
            raise ValueError("request exceeds byte limit")
        amount = ((Decimal(body_bytes) + 4096) * INPUT_PRICE
                  + OUTPUT_TOKENS * OUTPUT_PRICE) / 1_000_000
        with self.lock:
            reserved = sum(self.reservations.values(), self.uncertain)
            if self.calls >= self.max_calls or self.spent + reserved + amount > self.limit:
                raise ValueError("evaluation budget exhausted")
            self.calls += 1
            self.reservations[self.calls] = amount
            return self.calls

    def settle(self, ticket, cost):
        with self.lock:
            if ticket not in self.reservations:
                raise ValueError("unknown or already settled ticket")
            if cost is None:
                self.uncertain += self.reservations.pop(ticket)
                return
            try:
                value = Decimal(str(cost))
            except InvalidOperation as error:
                raise ValueError("invalid reported cost") from error
            if not value.is_finite() or value < 0:
                raise ValueError("invalid reported cost")
            self.reservations.pop(ticket)
            self.spent += value

    def snapshot(self):
        with self.lock:
            return {"calls": self.calls, "spent_usd": str(self.spent),
                    "reserved_usd": str(sum(self.reservations.values(), self.uncertain)),
                    "limit_usd": str(self.limit)}
