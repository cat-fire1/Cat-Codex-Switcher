# Encapsulate Account Storage and Credential Staging

Encapsulate `Store` internal fields and filesystem paths behind the `AccountRepository` interface using RAII-based `StagedEnrollment` for browser logins and unified `record_usage` persistence. Callers interact with profile models and credentials as domain entities rather than manipulating disk paths, DACL permissions, and temporary directories directly. This localizes Windows security invariants and failure rollbacks inside the repository module.
