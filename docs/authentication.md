# Browser Authentication

Catalog keeps account identity separate from authentication providers. The `users` and
`workspace_memberships` tables contain no provider-specific identifier. This release
adds local-password credentials, email-action persistence, and browser sessions.
External identity adapters remain separate follow-up work.

## External-provider identity adapter seam

External providers (including future OIDC and SAML adapters) are isolated from
Catalog accounts by `external_identities`. The stable provider key is the exact
verified `(issuer, subject)` pair, which is globally unique and links to one
ordinary internal `users.id`. Provider-specific identifiers and claims never
appear on `users` or `workspace_memberships`.

An adapter validates its protocol callback before resolving the verified pair
with `find_external_identity_user`. It must not merge identities by email. A
new provider identity may be linked only by an explicit authenticated
account-linking action through `link_external_identity`. The resulting user
uses the same sessions, invalidation, membership, and RBAC evaluation as a
local account.

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
to five failures per normalized email in fifteen minutes.

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
roles. The CLI reads the bearer secret from `CATALOG_TOKEN` (or `--token`).
