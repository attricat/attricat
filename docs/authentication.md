# Browser Authentication

Catalog keeps account identity separate from authentication providers. The `users` and
`workspace_memberships` tables contain no provider-specific identifier. This release
adds local-password credentials, email-action persistence, and browser sessions.
External identity adapters remain separate follow-up work.

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

There is no email provider. A delivery adapter must send the opaque secret without
recording it in logs, telemetry, database rows, or API responses.

## Browser sessions

`POST /auth/login` verifies a local password and sets an opaque `catalog_session`
HttpOnly cookie plus a separate `catalog_csrf` synchronizer-token cookie. The API
stores SHA-256 digests only. `POST /auth/renew` atomically revokes the old identifier
and replaces both values; login also revokes older workspace sessions. `POST
/auth/logout` revokes the current session and clears both cookies. Sessions expire
after eight hours and are revoked on account/password changes, membership changes,
and role-grant changes.

Production cookies are `Secure`, `HttpOnly` (session only), `SameSite=Lax`, and
path-scoped to `/`. `SESSION_COOKIE_SECURE=false` is exclusively for local HTTP
development and test servers. Every cookie-authenticated unsafe request must send
`X-Catalog-Csrf` equal to the current CSRF cookie. Login attempts are durably limited
to five failures per normalized email in fifteen minutes.
