# Fixture: Django URL conf.
from django.urls import path, re_path

from . import views

urlpatterns = [
    path("orders/", views.create_order),
    path("orders/<int:pk>/", views.order_detail, name="order-detail"),
    path("legacy/", LegacyView.as_view()),
    re_path(r"^archive/(?P<year>[0-9]{4})/$", views.archive),
    path(prefix, views.dynamic),
]
