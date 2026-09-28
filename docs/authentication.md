# Browser Authentication

Catalog keeps account identity separate from authentication providers. The `users` and
`workspace_memberships` tables contain no provider-specific identifier. Local
password credentials, email actions, and browser sessions are implemented;
external identity providers are not.

## External-provider identity adapter seam

External providers (including future OIDC and SAML adapters) are isolated from
Catalog accounts by `external_identities`. The stable provider key is the exact
verified `(issuer, subject)` pair, which is globally unique and links to one
ordinary internal `users.id`. Provider-specific identifiers and claims never
appear on `users` or `workspace_memberships`.

A future adapter would need to validate its protocol callback before
resolving the verified pair through the Rust account repository. It must not
merge identities by email; linking a new provider identity would require an
explicit authenticated account-linking action. No OIDC/SAML callback or
account-linking flow is available today.

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

The Rust account repository owns access to global `users`, credentials, and
action-token tables. It creates accounts, looks up password credentials, issues or
revokes lifecycle actions, and consumes valid verification or password actions in
explicit transactions. Adapters pass only password hashes and token digests to that
application layer; database migrations contain no authorization or lifecycle functions.

The SMTP delivery adapter uses Mailpit for local development and E2E testing.
It builds reset links from `PASSWORD_RESET_URL` and must not record opaque
secrets in logs, telemetry, database rows, or API responses. Mailpit's web UI
is for local inspection; E2E retrieves captured messages and URLs through its
REST API. Production uses a separately operated SMTP service with TLS; see
[production operations](operations.md#smtp-and-secret-rotation).

## Workspace sign-in routing

Each workspace has an immutable, unique `login_identifier`. It is trimmed and
lowercased at the API boundary and must be a 3–253 character domain-like name;
it is a sign-in routing key only, so Catalog performs no DNS lookup, ownership
verification, or host routing. Existing bootstrap workspaces receive
`<slug>.local` during migration.

Unauthenticated clients first call `POST /auth/discover` with
`{"login_identifier":"example.local"}`. On success it returns only the
normalized identifier and `sign_in_methods` (currently `["local_password"]`),
which is the compatibility seam for OIDC, SAML, and passkeys. Discovery is
rate-limited and unknown workspaces get a generic not-found response.
`POST /auth/login` then requires `login_identifier`, `email`, and `password`.
The server resolves the identifier and issues a session for that workspace;
clients never select a workspace ID. The web UI uses `/login` then the stable
workspace-specific `/login/:identifier` credential URL so password managers
can retain separate native username/current-password credentials.

## Password reset

`POST /auth/password-reset` accepts an email and always returns `204 No Content`,
regardless of account eligibility, to resist account enumeration. Eligible active
users with verified email and a local credential receive a 30-minute, one-time
`password_reset` link. `POST /auth/password-reset/confirm` accepts its opaque
token and a new password, hashes it with Argon2id, and atomically consumes the
link. Invalid, expired, revoked, or replayed links produce the same generic
validation error. Reset never creates a browser session; users sign in normally
afterward. The credential/security version changes revoke existing sessions and
outstanding lifecycle links.

Custom domains, SSO enforcement/configuration, SCIM/JIT provisioning, MFA,
passkeys, and switching workspaces after login are explicitly deferred.

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
to five failures per normalized workspace/email pair in fifteen minutes.

## Personal API tokens

`POST /personal-access-tokens` issues an opaque `cat_pat_...` bearer secret for
an authenticated principal with `tokens.manage`. The secret is returned exactly
once; storage contains only its SHA-256 digest. Requests authenticate it using
`Authorization: Bearer <secret>`, and its workspace is selected by the token,
not a caller-controlled header.

Tokens have a label, non-empty explicit permission subset, optional expiry,
revocation time, and last-used time. `GET /personal-access-tokens` deliberately
returns metadata only and `DELETE /personal-access-tokens/{id}` revokes only the
caller's token. Token permissions restrict the owner’s RBAC grants rather than
replace them, so a token cannot exceed its owner’s grants or bypass scoped
roles. Role grant and revoke requests require both `members.manage` and
`roles.grant` in the token as well as the owner's live grants. When requested
with a personal token, `/auth/session` reports capabilities restricted to that
token's permissions. The CLI reads the bearer secret from `CATALOG_TOKEN` (or
`--token`).
