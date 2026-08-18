// Fixture: a TSX React component.
import React, { useState } from "react";
import axios from "axios";

export function CheckoutButton({ orderId }: { orderId: string }) {
    const [busy, setBusy] = useState(false);

    async function onSubmit() {
        setBusy(true);
        await axios.post(`/api/orders/${orderId}`, { paid: true });
        setBusy(false);
    }

    return <button disabled={busy} onClick={onSubmit}>Buy</button>;
}

export default CheckoutButton;
