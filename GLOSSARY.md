# Cat Codex Switcher

A Windows desktop application for securely managing multiple Codex accounts, switching active credentials, and tracking usage rate limits in the system tray.

## Language

**Account Switch**:
The transactional process of safely replacing the active login credentials with a chosen profile while coordinating the Codex Desktop application lifecycle.
_Avoid_: handoff, account change, profile swap

**Active Login**:
The credentials file currently placed in `CODEX_HOME/auth.json` and consumed by the active Codex runtime and Desktop session.
_Avoid_: current credential, live account, working login

**Saved Profile**:
A persisted account record and isolated copy of credentials in the application's private secure storage.
_Avoid_: account record, profile entry, stored account

**Account Repository**:
The deep module responsible for secure storage, DACL permission isolation, corruption recovery, and persistence of profiles, settings, and cached rate limits.
_Avoid_: data store, storage manager, database service

**Credential Staging**:
An isolated, DACL-protected temporary working directory used to create, verify, or swap credentials before committing to the repository or rolling back on failure.
_Avoid_: temp auth dir, intermediate home, scratch directory

**Desktop Controller**:
An adapter managing the discovery, graceful termination, and relaunch of the Codex Desktop process.
_Avoid_: desktop manager, process handler, application supervisor

**Codex Client**:
The deep module managing communication with the local Codex runtime, encapsulating IPC session lifecycle and rate limit telemetry.
_Avoid_: rpc session, app-server connection, codex bridge

**Runtime RPC**:
The JSON-RPC communication protocol carried over standard input/output streams with the local `codex app-server` child process.
_Avoid_: cli bridge, pipe transport, stdio stream

**Backend Transport**:
The adapter seam decoupling the TypeScript user interface from the Rust core, satisfied by a production `TauriTransport` and a development/test `MockTransport`.
_Avoid_: api client, invoke wrapper, backend service

**UI Controller**:
The module managing reactive view state, user intent concurrency locking, error capturing, and snapshot synchronization.
_Avoid_: state manager, view model, action coordinator
