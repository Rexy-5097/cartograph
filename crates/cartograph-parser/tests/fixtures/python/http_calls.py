# Fixture: HTTP observations (never resolved, never edges).
import httpx
import requests

BASE = "/api/v1"
url = "/computed"
config = {}


requests.get("/orders")
requests.post("/orders", json={})
requests.put("/orders/1")
requests.delete("/orders/1")
requests.post(url)
httpx.get("/health")
httpx.post(f"{BASE}/orders/{order_id}")
client.get("/from-client")
session.post("/from-session")
api.post("/orders/from-unknown-object")

config.get("timeout")          # dict lookup: NOT an HTTP observation
os.environ.get("HOME")         # env lookup: NOT an HTTP observation
cache.get(key)                 # unknown object, non-path arg: NOT observed
