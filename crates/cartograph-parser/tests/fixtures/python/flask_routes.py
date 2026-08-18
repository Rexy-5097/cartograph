# Fixture: Flask decorator routes.
from flask import Blueprint, Flask

app = Flask(__name__)
bp = Blueprint("orders", __name__)

ALLOWED = ["PUT"]


@app.route("/orders", methods=["POST"])
def create_order():
    pass


@app.route("/orders", methods=["GET", "POST"])
def orders():
    pass


@app.route("/health")
def health():
    pass


@app.route("/dynamic", methods=ALLOWED)
def dynamic_methods():
    pass


@bp.route("/blueprint/items", methods=["DELETE"])
def blueprint_items():
    pass
