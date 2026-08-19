from fastapi import FastAPI
app = FastAPI()

@app.post("/api/v1/orders")
async def create_order(payload: dict):
    pass

@app.get("/api/v1/orders/{id}")
async def get_order(id: str):
    pass
