# Development

- [external-storage.md](external-storage.md) — working from the external SSD,
  cargo target-dir, exFAT caveats
- Root [Makefile](../../Makefile) — `make help`
- [CONTRIBUTING.md](../../CONTRIBUTING.md) — workflow, branch/commit conventions
- [AGENTS.md](../../AGENTS.md) — AI assistant working agreement

Machine profile this project is tuned for: MacBook Air M4, 16 GB RAM, 256 GB
internal SSD, 1 TB external SSD. No Docker, no local LLMs; rust-analyzer alone
claims 3–6 GB and the chassis is fanless — keep heavyweight processes few.
