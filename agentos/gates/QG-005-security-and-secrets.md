# QG-005 — Security & secrets

Scans every git-tracked file (vendored `agentos/` included) for:

- Credential patterns: `ghp_`/`gho_` tokens, AWS `AKIA…` keys, private key
  blocks, `.env` files.
- Machine-specific absolute paths (`/Users/...`, `/Volumes/...`, `C:\Users\...`)
  outside documented placeholders (RULE: no machine paths committed).
- `.env`, `*.pem`, `*.key`, `credentials.json` must not be tracked.

Also (by review at M00, mechanically later): logging code paths never emit
source contents, env values or tokens (RULE 015).
