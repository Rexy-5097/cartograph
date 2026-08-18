// Fixture: symbol declarations and export forms.
const BASE = "/api/v1";
let counter = 0;

export function plain(): void {}

export async function fetchOrders(): Promise<void> {}

function inner_host(): void {
    function nested(): void {}
}

export class Cart {
    total = 0;
    add(amount: number): void {
        this.total += amount;
    }
    async sync(): Promise<void> {}
}

export interface Order {
    id: string;
}

export type OrderId = string;

export enum Status {
    Open,
    Closed,
}

class Hidden {}

export { BASE };
export default Hidden;
