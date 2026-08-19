import axios from "axios";
import { API_BASE } from "./config";

export async function create() {
    await axios.post(`${API_BASE}/orders`, {});
}
export async function get(orderId: string) {
    await axios.get(`${API_BASE}/orders/${orderId}`);
}
export async function envy(id: string) {
    await axios.get(`${process.env.API_URL}/orders/${id}`);
}
