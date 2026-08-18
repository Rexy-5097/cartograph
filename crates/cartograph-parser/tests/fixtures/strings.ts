// Fixture: string and template structure. Nothing here is "resolved".
const BASE = "/api/v1";
const plain = "hello world";
const empty = "";
const escaped = "line\nbreak";
const template = `${BASE}/orders/${orderId}`;
const noSubs = `just text`;
const joined = "pre" + BASE + "post";
const numeric = 1 + 2; // no string operand: not a concatenation fact
declare const orderId: string;
