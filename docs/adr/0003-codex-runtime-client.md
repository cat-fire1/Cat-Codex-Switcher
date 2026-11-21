# Deep Codex Runtime Client with Pluggable Adapters

Encapsulate `codex app-server --stdio` subprocess orchestration, Windows creation flags, JSON-RPC framing, and Tokio runtime lifecycle behind a synchronous `CodexClient` interface. Provide two adapters at the seam: `ProcessCodexClient` for production execution and `InMemoryCodexClient` for fast in-memory unit tests. This eliminates external subprocess spawning from unit tests and removes Tokio runtime propagation from domain callers.
