# Local Account Lifecycle

Catalog keeps account identity separate from authentication providers. The `users` and
`workspace_memberships` tables contain no provider-specific identifier. This release
adds the local-password and email-action persistence boundary; browser sessions,
cookie transport, login/logout endpoints, and external identity adapters remain
separate follow-up work.

## Local credentials

A user has at most one `local_password_credentials` row. It stores an Argon2id PHC
hash, timestamps, and a monotonically increasing credential version. Plaintext
passwords are accepted only by the account layer and must never be persisted,
serialized, traced, or logged.

The account layer generates 32-byte opaque lifecycle secrets for delivery and stores
only their SHA-256 digest. Secrets are URL-safe base64url values and are deliberately
not `Debug` or serializable application values.

## Email actions

`user_lifecycle_action_tokens` supports these one-time purposes:

- `email_verification`
- `password_setup`
- `password_reset`

Each action captures the account security version and credential version at issuance,
expires at a fixed timestamp, and can be consumed once. Issuing a newer action for
the same user and purpose revokes the prior one. Password setup and reset require a
verified, active email; setup requires no credential and reset requires an existing
credential.

Verification, password changes, user-state changes, and security-version changes
revoke outstanding lifecycle actions. Password changes also advance the credential
version. Consumers must atomically select and mark an action consumed so concurrent
requests cannot replay a link.

## Persistence boundary

The request database role has no direct access to global `users`, credentials, or
action-token tables. It uses narrowly scoped database functions to create an account,
look up a password credential, issue/revoke lifecycle actions, and consume valid
verification or password actions. Adapters must pass only the password hash and token
digest to those functions.

There is no email provider or HTTP endpoint in this release. A delivery adapter must
send the opaque secret without recording it in logs, telemetry, database rows, or API
responses. Cookie sessions and their invalidation contract are intentionally deferred
to the browser-authentication work.
