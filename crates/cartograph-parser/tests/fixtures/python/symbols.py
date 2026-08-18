"""Fixture: declarations, nesting and __all__."""

BASE = "/api/v1"
_private_const = 3
__all__ = ["fetch_orders", "Cart"]


def plain():
    pass


async def fetch_orders():
    pass


def outer():
    def nested():
        pass
    return nested


class Cart:
    limit = 10

    def add(self, amount):
        self.total += amount

    async def sync(self):
        pass

    @staticmethod
    def helper():
        pass


class _Hidden:
    pass
