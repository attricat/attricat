# Hardening review

This review focused on major authorization, data-exposure, concurrency and
resource-exhaustion risks. It covered request authentication, role delegation,
agent execution, approval and stream revocation, relationship reads, file
access, archive parsing, and selected worker, storage and frontend request/cache
boundaries. It was not
an exhaustive audit of every source file.

## Fixed findings

- **Personal-token privilege escalation:** role edits, grants, invitations,
  replacement roles and ownership transfers could delegate the owner's
  permissions instead of the token's narrower set. Delegation now checks the
  token inside the mutation transaction and keeps referenced roles stable.
- **Agent privilege escalation:** deferred tools lost their initiating token,
  and agent-only users could approve another user's privileged mutation. Runs
  now persist and recheck the credential, preserve it in audit attribution, and
  require the approver's mutation permission. Record-bound conversations and
  smart-fill also enforce the token's read permission.
- **Related-record disclosure:** previews, hierarchy and incoming relationships
  could expose labels from unreadable records. Hydration now checks each
  related record. Incoming pagination still advances over filtered rows.
- **Missing file authorization:** file-retention-hold reads skipped the deferred
  file-access check. They now use the same record and deployment policy checks
  as file metadata.
- **Login throttling bypass:** changing the workspace reset the guessing budget
  for a global account credential. The normalized account address now shares
  one budget across workspaces.
- **Ownership race and destructive role replacement:** concurrent transfers
  could reuse an owner's stale authority; retiring a role into itself deleted
  its grants. Ownership is rechecked after locking, and self-replacement is
  rejected.
- **Archive resource exhaustion:** tar metadata and trailing decompression
  escaped payload limits, and cancelled requests released admission while
  blocking parsing continued. The entire expanded stream and concurrent
  validation work are now bounded, including after cancellation.
- **Preview resource exhaustion:** per-field limits did not bound a branching
  graph. Global fetched-row and path-expansion budgets now fail oversized reads
  with a structured client error.

- **Stale worker writes:** agent comments, saved searches and publications
  could commit after another worker reclaimed their task. These mutations now
  use the shared fenced transaction helpers, rolling back data, audit evidence
  and publication events together. Saved-search mutations also use the shared
  audit path.
- **Expired leases during lock waits:** the shared task fence used PostgreSQL's
  transaction-start timestamp. It now checks wall-clock time, so a transaction
  that waited on a domain lock cannot commit with an expired lease.
- **Competing agent resumes and lost approvals:** every sibling decision could
  queue a separate task, allowing competing workers to mark a healthy run as
  interrupted. Early decisions could also strand a run awaiting approval.
  Decisions now serialize on the run, with one resume queued after the whole
  response is decided. Terminal runs reject new decisions and disappear from
  pending-approval lists; repeat decisions retain their conflict response.

- **Agent stream revocation gap:** an open event stream could continue reading
  new events after credential expiry or revocation, removal of `agents.run`,
  or loss of access to its conversation's record. Streams now recheck live
  credentials and permissions before exposing each batch, including idle
  polls, and terminate with an error when access is lost.

- **Worker heartbeat deadlock:** awaiting lease renewal inside a timer branch
  stopped polling the job. File processing could then deadlock because the job
  held the lease row that renewal needed, even after the original file lock
  was released. File and shared task workers now poll execution and renewal
  concurrently, and drop execution resources before lease-loss cleanup.

- **Invitation-based access restoration:** an older pending invitation could
  reactivate a disabled member or restore a revoked role grant, including when
  acceptance raced with deactivation. Access revocation now invalidates pending
  invitations in that workspace. Invitation creation, acceptance, onboarding and
  revocation share the workspace lock before taking invitation or membership
  locks. New administrator-issued invitations and other workspaces' invitations
  remain usable.

- **Stale role and owner delegation:** grants and invitations could use a role
  that gained permissions during a lock wait, or owner authority lost to a
  concurrent transfer. Role creation, editing, retirement and member mutations
  now share workspace-level serialization and recheck authority inside the
  transaction. Grant and invitation checks use the locked role's current
  permissions. Role edits cannot restore authority removed while they waited,
  and retirement cannot use a former owner's authority to create owner grants.

- **Session renewal escaping revocation:** a revocation waiting on a renewing
  session could miss its newly inserted replacement because the revocation's SQL
  statement used an earlier snapshot. The replacement survived a role revocation
  or became usable after a disabled member was reactivated. Renewal now takes
  the workspace lock before the session row, serializing replacement creation
  with membership and role revocation.

Regression coverage is in `apps/api/tests/security_hardening.rs`,
`apps/api/tests/worker_hardening.rs`, `apps/api/tests/agent_stream_security.rs`,
`apps/api/tests/membership_revocation.rs`, `apps/api/tests/role_delegation_races.rs`,
`apps/api/tests/session_revocation_races.rs`, existing agent, personal-token,
lifecycle and controlled-record tests, and HTTP/archive
unit tests. Worker lock contention and cancellation are covered in
`crates/workers/src/file_worker.rs` and `crates/workers/src/heartbeat.rs`.

## Upgrade and client notes

The SQLx migration adds durable agent credential provenance. Existing unfinished
runs without provenance fail closed and must be resubmitted; they cannot safely
be assumed to have originated from a browser session.

Restricted tokens must include the permissions they delegate. Agent approval
requires the underlying mutation permission. When a provider proposes several
mutations together, all proposals must be approved or rejected before execution
resumes. Relationship pages can contain no
visible items and still have a `next_cursor`; clients must follow the cursor.
See [API read safety](api.md#relationship-read-safety) for preview budgets.

For access revoked before this upgrade, review and revoke the affected users'
remaining pending invitations. The invitation fix invalidates links during
subsequent access revocations; it does not rewrite historical invitation records.
If a browser session may already have escaped an earlier concurrent revocation,
reset that account's password to invalidate its browser sessions across workspaces.

## Verification scope

Verification uses the Rust workspace tests, focused security regressions,
frontend unit tests, Clippy, formatting, dependency-policy checks and the
worktree's readiness endpoint. The clean, non-overlapping workspace run passed
864 Rust tests (3 opt-in tests ignored); all 815 frontend unit tests passed.
Workspace Clippy, formatting, dependency-policy checks and readiness passed.
This verification ran after rebasing onto `main` at `77ccaf1`, including its
demo-mode changes.
An earlier follow-up reviewed record-batch authorization and atomicity, context
mutation locking, controlled-record protections, and file download/cache handling
without finding an additional confirmed major issue. A credential lifecycle
review reproduced the invitation restoration gap in three failing regressions
before fixing it, including concurrent acceptance. A subsequent review reproduced
stale role and ownership delegation in four failing regressions; six lock-controlled
tests cover grants, invitations, role editing and retirement. The latest round
reviewed file cleanup/retention, workflow action retries and browser-session
lifecycle boundaries. Two new failing regressions reproduced renewal escaping
role revocation and member deactivation; they pass after the fix and also verify
that fresh login remains supported. Passing tests are not a security guarantee.

The sibling `../../attricat-extension-example` checkout was unavailable, so its
specific package/side-load/enable/event workflow could not be exercised.
Repository extension integration tests do exercise packaged Wasm components.
Browser Playwright, the opt-in S3 compatibility probe and controlled performance
benchmarks were not run.
