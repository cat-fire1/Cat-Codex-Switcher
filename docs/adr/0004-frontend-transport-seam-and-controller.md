# Decouple Frontend Transport and UI State Controller

Replace repeated `isTauri()` runtime checks with a unified `BackendTransport` seam satisfied by `TauriTransport` and `MockTransport` adapters. Encapsulate user interaction concurrency guards, error capture, and snapshot synchronization in a deep `useAccountSwitcher` UI controller hook. This makes view components strictly declarative and allows deterministic testing of frontend state transitions without mocking global window properties.
