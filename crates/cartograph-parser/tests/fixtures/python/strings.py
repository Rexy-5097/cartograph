# Fixture: string structure. Nothing here is evaluated.
BASE = "/api/v1"
plain = "hello world"
empty = ""
escaped = "line\nbreak"
triple = """multi
line"""
fstring = f"/orders/{order_id}"
nested_f = f"{BASE}/orders/{order_id}/items"
no_subs = f"just text"
joined = "pre" + BASE + "post"
numeric = 1 + 2
