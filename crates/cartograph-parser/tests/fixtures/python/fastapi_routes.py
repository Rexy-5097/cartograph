# Fixture: FastAPI decorator routes.
from fastapi import APIRouter, FastAPI

app = FastAPI()
router = APIRouter()


@app.get("/orders")
def list_orders():
    pass


@app.post("/orders")
async def create_order(payload):
    pass


@app.put("/orders/{id}")
def replace_order(id):
    pass


@app.delete("/orders/{id}")
def remove_order(id):
    pass


@router.get("/health")
def health():
    pass


@router.post("/router/items")
def add_item():
    pass


@app.middleware("http")
async def add_timing(request, call_next):
    pass


@staticmethod
def not_a_route():
    pass
