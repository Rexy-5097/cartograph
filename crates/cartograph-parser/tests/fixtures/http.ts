// Fixture: HTTP-looking observations (never resolved, never edges).
declare const axios: any;
declare const client: any;
declare const routes: Map<string, string>;
declare const orderId: string;
const BASE = "/api/v1";

fetch("/orders");
fetch("/orders", { method: "POST" });
fetch("/orders", { method: dynamicMethod });  // non-literal: hint stays unknown
axios.get("/orders");
axios.post(`${BASE}/orders/${orderId}`, {});
client.get("/health");
routes.get("lookup-key");        // map access: NOT an HTTP observation
widget.get("/orders/latest");    // unknown object, path-like arg: observed
declare const widget: any;
declare const dynamicMethod: string;
