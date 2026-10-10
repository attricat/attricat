# Browser Authentication

Attricat keeps account identity separate from authentication providers. The `users` and
`workspace_memberships` tables contain no provider-specific identifier. Local
password credentials, email actions, and browser sessions are implemented;
external identity providers are not.

## External-provider identity adapter seam

External providers (including future OIDC and SAML adapters) are isolated from
Attricat accounts by `external_identities`. The stable provider key is the exact
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
[Rotate secrets](../apps/docs/src/content/docs/operate/deployment.md#rotate-secrets).

## Workspace sign-in routing

Each workspace has an immutable, unique `login_identifier`. It is trimmed and
lowercased at the API boundary and must be a 3–253 character domain-like name;
it is a sign-in routing key only, so Attricat performs no DNS lookup, ownership
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

`POST /auth/login` verifies a local password and sets an opaque `attricat_session`
HttpOnly cookie plus a separate `attricat_csrf` synchronizer-token cookie. The API
stores SHA-256 digests only. `POST /auth/renew` atomically revokes the old identifier
and replaces both values; login issues fresh session and CSRF credentials. `POST
/auth/logout` revokes the current session and clears both cookies. Sessions expire
after eight hours and are revoked on account/password changes, membership changes,
and role-grant changes. Renewal shares the workspace lock used by membership
and role revocation, so a replacement session cannot escape a concurrent
revocation or become valid again when a disabled member is reactivated.

Deactivating a member or revoking a role grant also revokes that user's pending
invitations and onboarding links in the affected workspace. Acceptance is
serialized with revocation, so an older invitation cannot restore that access.
Invitations for other workspaces are untouched. A new administrator-issued
invitation can authorize access again.

Production cookies are `Secure`, `HttpOnly` (session only), `SameSite=Lax`, and
path-scoped to `/`. `SESSION_COOKIE_SECURE=false` is exclusively for local HTTP
development and test servers. Every cookie-authenticated unsafe request must send
`X-Attricat-Csrf` equal to the current CSRF cookie. Login attempts are durably limited
to five failures per normalized account email in fifteen minutes, shared across
workspaces.

## User preferences

`PATCH /auth/preferences` replaces the authenticated user's display
preferences. The request body currently has one required field, `time_zone`: an
IANA zone name validated by the API (for example `Europe/Warsaw` or `UTC`), or
`null` to follow each client's own zone. Unknown names return `422`. The
preference is stored on the user account, so it applies across workspaces,
devices, and sessions, and it is returned as `time_zone` in every session
payload. It only affects rendering: the API stores and returns instants in UTC,
and exports keep UTC ISO timestamps. Cookie-authenticated requests need the
CSRF header like any other unsafe request.

## Display name

`PUT /auth/display-name` changes the authenticated user's display name. The
request body has one required field, `display_name`: 2 to 64 Unicode letters,
digits, and spaces, where spaces may only appear between other characters (no
leading or trailing space). The API does not trim the value; invalid names
return `422`. The name is stored on the user account, so it applies across
workspaces, and the response is the updated session payload. Cookie-authenticated
requests need the CSRF header like any other unsafe request.

## Avatars

Each workspace membership can have an avatar, so a member who belongs to
several workspaces sets one per workspace. `PUT /auth/avatar` accepts a
`multipart/form-data` body with exactly one `file` part: a PNG or JPEG image of
at most 10 MiB (or `FILE_UPLOAD_MAX_BYTES` when that is lower), identified by
its content signature. Other formats return `415`. The avatar is an ordinary
workspace file with `purpose = 'avatar'`: the upload is queued for the file
worker, which applies EXIF orientation, center-crops the image to a square,
flattens any transparency onto white, and stores a 256×256 `avatar` WebP
variant (smaller images are cropped but not upscaled). The response is `201` with
`{ "file_id", "status" }`. `DELETE /auth/avatar` removes the avatar and returns
`204`. A replaced or removed avatar is no longer referenced, so file
reconciliation reclaims it.

The session payload's `avatar` field is the caller's own avatar in the session
workspace, including one that is still `queued` or `processing`. Member lists,
audit events, and record change history expose `avatar_file_id`,
`actor_avatar_file_id`, or `approved_by_avatar_file_id` only once the avatar is
`ready` and the member is active. Any authenticated member of the workspace
can download a current avatar with
`GET /files/{file_id}/variants/avatar/download`. The original upload, its
metadata, and other variants are never served for avatar files, because the
original may carry EXIF data such as location. Avatar files cannot be linked to
record file attributes. Deployments can restrict avatar operations through the
`AvatarUpload` and `AvatarRead` file access policy operations.

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
token's permissions. When a PAT issues another PAT, the requested permissions
must also be a subset of the issuing token's permissions. If the issuer expires,
the new token must have an explicit expiry no later than its issuer. Revocation
remains per-token; revoke separately issued tokens individually. User state is
checked during credential validation, and authenticated session/profile routes
also require an active workspace membership. `last_used_at` is updated at most
once per minute rather than on every request.

Role creation, editing, duplication, replacement, invitations, grants, and
ownership transfer cannot delegate permissions omitted from the request token.
For both browser sessions and tokens, role and membership changes serialize on
the workspace. Management authority, ownership and delegated permissions are
checked inside the mutation transaction, after lock waits, against the current
role definitions and grants.

Agent runs retain their initiating token and recheck its live permissions for
reads and approved mutations, including after a worker restart. Approving an
agent mutation also requires the approver's own permission for that operation;
`agents.run` alone is not approval authority. Queued or approval-waiting runs
created before credential binding was introduced fail closed when next executed;
resubmit those requests after upgrading. Their missing token provenance cannot
safely be treated as browser-session authority.

The CLI reads the bearer secret from `ATTRICAT_TOKEN` (or `--token`).

## Attribute visibility

Authorization applies to whole records: a person who can read a record reads
all of its attribute values, history included. See
[Field-level read restrictions](field-level-read-restrictions.md) for the audit
of every read surface and the design for restricted attributes.
