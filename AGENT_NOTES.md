# Agent Notes — Arb Bot Project

## Attempt 1: Raw Ollama + kimi-k2.6:cloud (FAILED)
- **Date**: 2026-04-23
- **Method**: Single prompt file → `ollama run kimi-k2.6:cloud < prompt.txt`
- **Failure**: Model context window exceeded. Combined prompt+output hit 355,961 tokens against 262,144 max.
- **Root cause**: Model generated ~617KB of inline "thinking" and reasoning before emitting code, consuming context budget. Never reached file emission with `// path:` headers.
- **Output size**: 630,855 bytes, 4,525 lines — mostly reasoning, no extractable files.
- **Remediation for Attempt 2**: Switch to qwen3-coder:480b-cloud or break into chunked generation.

## Rules
- Max 2 attempts per issue.
- Document in AGENT_NOTES.md.
- Never touch working code from other projects.
- Cap terminal output with | tail -15 on cargo/grep.

## 2026-04-24 Build Status
- cargo check --workspace: PASS
- cargo build --release --workspace: PASS
- cargo test --workspace: 1 failure (kraken signing test exact match) - test data has a base64 decode issue with the secret string. Binance signing test passes.
