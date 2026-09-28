# Optional Solution-Pack Sample Data

> **Status: implemented.** Format 1 is available through the administrator-only
> solution-pack inspect, plan, and apply workflow described below.

## Decision

A pack may contain one optional synthetic sample-data declaration.
An administrator must select it explicitly with `--include-sample-data` while
creating a plan. It is never selected by the archive, by inspection, by a
pack-defined default, or by `apply`; apply continues to accept only an immutable
plan ID.

Selected samples create ordinary workspace entities. They use ordinary entity
validation, audit, and `entity.created.v1` dispatch, and may therefore trigger
enabled workflows or extensions. Both archive inspection and plan review must
show a prominent warning that selecting samples can run automation and cause
external effects.

This decision does not add pack ownership, entity publication, updates,
deletes, rollback, entity cleanup, uninstall, or continuing reconciliation.

## Declaration Boundary

The strict manifest may index at most one sample resource:

```json
{
  "resources": {
    "sample_data": {
      "key": "sample-data/default",
      "path": "sample-data/sample-data.json",
      "sha256": "<lowercase-sha256>"
    }
  }
}
```

The declaration is strict JSON with `format_version: 1`,
`kind: "solution_pack_sample_data"`, the required attestation
`classification: "synthetic"`, and an ordered `entities` array. The published
schema is [`solution-pack-sample-data-v1.schema.json`](../contracts/solution-pack-sample-data-v1.schema.json).
Its exact authoring shape is:

```json
{
  "format_version": 1,
  "kind": "solution_pack_sample_data",
  "classification": "synthetic",
  "entities": [{
    "key": "sample-entities/navy-shirt",
    "blueprint": "blueprints/product",
    "facts": [{
      "attribute": "blueprints/product/attributes/name",
      "value": "Sample Navy Shirt"
    }],
    "relationships": [{
      "attribute": "blueprints/product/attributes/category",
      "targets": ["sample-entities/apparel"]
    }]
  }]
}
```

Both entity arrays are required and may be empty. Relationship target arrays
must be non-empty. Unknown fields, null/array values, arbitrary objects,
duplicate attribute facts, and duplicate targets are rejected. The sole object
value is the exact ordinary native-time shape
`{"time": <string>, "time_zone": <string>}` on an attribute resolved as
`time`; both keys are required and no other key is allowed. Authors cannot
declare a label, tag, metadata, context, ID, or publication field; visible labeling is
the host-owned marker described below. The first implementation is limited to
one 1 MiB file, 256 entities, 128 facts per entity, and 4,096 total scalar and
relationship facts.

Each entity has an immutable logical key matching
`^sample-entities/[a-z][a-z0-9_-]*$`, a logical entity-blueprint reference, scalar
facts addressed by logical attribute reference, and relationships. Blueprint
references must resolve to entity blueprints declared by the same pack and to
the exact published revision selected by the plan. Values are limited to the
ordinary native scalar types supported by that attribute, plus the exact
bounded native-time object described above; `null`, every other object, arrays,
files, binary values, and read-only attributes are rejected. Native-time
objects are accepted only for a resolved `time` attribute, must pass the
ordinary time/time-zone parser, and run matcher-v1 over both contained strings.
Duplicate facts and duplicate relationship targets are rejected.

Relationships may target only logical entity keys declared in the same sample
file. UUIDs, existing-entity references, cross-file or cross-pack references,
self-references, and cycles are rejected. The planner uses a deterministic
DAG order with relationship targets before sources. It does not introduce a
post-create relationship update phase.

## Synthetic Trust Standard

`classification: "synthetic"` is the pack author's attestation that values were
created for demonstration and were not copied or derived from customer or
production data. Catalog combines that attestation with strict structural and
syntactic rejection, trusted distribution review, and the applying
administrator's review. Archives from an untrusted or unreviewed source do not
meet this contract merely because they pass validation.

Validation rejects the entire archive, rather than warning or silently removing
content, when any prohibited field or value pattern is found. There is no
per-value override or allowlist escape. This deliberately accepts false
positives: an author must replace an ambiguous fixture with an obviously
synthetic value and publish a new archive.

Prohibited fields and references include:

- archive-assigned workspace, user, role, context, blueprint, entity,
  installation, release, or other database IDs;
- context keys, context IDs, hierarchy references, or non-default-context
  selectors;
- entity publication state, publication channels, created/updated timestamps,
  actor data, system metadata, system tags, or arbitrary tags; and
- file or binary attributes, object-store keys, upload handles, local paths,
  remote-download references, and signed asset references.

Validation covers the **complete effective initial fact set**, not only facts
written in the sample file. Planning first resolves inherited attributes and
materializes every ordinary default that entity creation would store, then runs
all field-name and value matchers over explicit and defaulted facts together.
A prohibited attribute code or an effective default blocks the archive when a
sample entity omits that attribute. An explicit fact suppresses its ordinary
default exactly as ordinary create does, so the suppressed default is neither
validated nor included in canonical evidence. The canonical plan input and its
digest include exactly the defaults ordinary create will materialize; apply
uses that reviewed input and the same exact blueprint revision.

### Prohibited matcher v1

`solution-pack-sample-prohibited-v1` is the immutable matcher for sample format
version 1. A later matcher that changes an accepted/rejected boundary requires
a new sample format version; deployments cannot silently tighten this matcher
for existing format-1 archives.

For each logical attribute code and effective scalar value, the matcher:

1. JSON-decodes strings, rejects invalid Unicode, normalizes to NFC, and creates
   a second ASCII-case-folded copy without locale-sensitive folding.
2. Rejects NUL, C0/C1 controls, Unicode noncharacters, CR or LF, and strings
   longer than 4,096 UTF-8 bytes.
3. Rejects any `%` not followed by two hexadecimal digits. When `%` occurs,
   performs exactly one percent-decoding pass, rejects invalid UTF-8, scans the
   decoded copy, and rejects it if any residual `%HH` escape remains. A
   double-encoded value therefore fails closed after one decode rather than
   receiving a second decode. JSON `\u` escapes cannot evade scanning.
4. Applies every rule below as a substring search unless the rule explicitly
   says that it covers the whole value. No heuristic score or configurable
   threshold is used.

String values receive every value matcher. Integer and finite-number values use
their canonical JSON decimal representation and receive the database-ID,
telephone, and encoded-blob matchers, preventing a numeric telephone bypass.
Booleans have no prohibited lexical form. Native date, time, and datetime values
receive strict ordinary type validation but are exempt from the string-only
RFC 3339 and telephone matchers. For the native-time object, `time` receives
that temporal exemption while `time_zone` receives the complete string matcher.

Attribute codes are split before case-folding at ASCII punctuation, whitespace,
and lower-to-upper camel-case boundaries. ASCII case-folding means replacing
only `A`–`Z` with `a`–`z`. Each token and the concatenation of all tokens are
rejected when equal to `password`, `secret`, `token`, `apikey`, `privatekey`,
`credential`, `authorization`, `session`, or `cookie`. This covers, for
example, `api_key`, `api-key`, and `apiKey` without an open-ended notion of
“secret-like.”

Value matching is defined as follows:

- **Database IDs:** reject a case-insensitive hexadecimal UUID token in exact
  `8-4-4-4-12` grouping, with either no braces or one matched brace pair and an
  optional `urn:uuid:` prefix; a 26-character token containing only
  `0-9A-HJKMNP-TV-Z` (Crockford ULID); or a 24-character hexadecimal
  Mongo/Object ID, at ASCII token boundaries.
- **Email:** reject a case-insensitive ASCII token matching one or more RFC
  5322 `atext` characters, `@`, one or more DNS labels, a dot, and a final
  2–63-letter label.
- **Telephone:** reject a maximal substring containing only digits, ASCII
  spaces, `.`, `-`, `(`, or `)`, optionally beginning with `+`, when it has
  7–15 digits and begins and ends at a non-digit boundary.
- **URI and host:** reject an ASCII scheme of 1–32 characters matching
  `[A-Za-z][A-Za-z0-9+.-]*`, followed by `:` and a non-whitespace character;
  a strict RFC 3986 IPv4 `dec-octet` or RFC 4291 IPv6 literal without a zone
  identifier; or an ASCII DNS name with at least two labels. Each DNS label is
  1–63 characters matching
  `[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?`, and the final label is 2–63
  ASCII letters.
- **DSN and paths:** reject case-insensitive `host=`, `server=`,
  `data source=`, or `dsn=` followed by a non-whitespace value; and reject a
  value beginning with `/`, `~/`, `./`, `../`, `\\`, or an ASCII drive letter
  plus `:\`. URI matching separately rejects object-store schemes.
- **Credentials:** reject a PEM private-key marker or envelope under the exact
  grammar below; three dot-separated base64url segments of at least eight
  characters each; case-insensitive `Basic`, `Bearer`, or `Token` followed by
  ASCII whitespace and a non-whitespace value; AWS access keys beginning
  `AKIA` or `ASIA` plus 16 uppercase letters or digits; GitHub tokens beginning
  `ghp_`, `gho_`, `ghu_`, `ghs_`, or `ghr_`; and Stripe keys beginning
  `sk_live_`.
- **Encoded blobs:** reject an ASCII hexadecimal token of at least 32
  characters, or a base64/base64url token of at least 32 characters whose
  length and padding are valid and which decodes to at least 24 bytes. This is
  the complete deterministic replacement for a high-entropy estimate.
- **Timestamps:** reject an RFC 3339 date or timestamp in a string attribute.
  Typed date, time, and datetime values remain allowed only when the resolved
  blueprint attribute requires that native type; metadata timestamps remain
  prohibited.

A PEM marker is exactly `-----BEGIN <label>-----` or
`-----END <label>-----`, where `<label>` is one of `PRIVATE KEY`,
`ENCRYPTED PRIVATE KEY`, `RSA PRIVATE KEY`, `EC PRIVATE KEY`,
`DSA PRIVATE KEY`, or `OPENSSH PRIVATE KEY`. Any such marker rejects, including
an unpaired or mismatched marker. For conformance, a complete envelope has a
BEGIN marker and END marker with the same label, 1–64 intervening lines, each
1–64 ASCII base64 characters. Only the final body line may end with one or two
`=` padding characters; no earlier `=` is allowed. The total normalized string
is at most the global 4,096-byte limit. LF or CRLF
may separate the grammar's lines; the general CR/LF rule also independently
rejects the value. These bounds make complete, partial, mismatched, and oversized
private-key fixtures deterministic and fail closed.

Token boundaries above are ASCII alphanumeric boundaries. Validators must use
the exact rules, normalization, and thresholds above rather than library email,
phone, URL, or entropy heuristics.

The table contains canonical decoded, unencoded seeds. For each rejected scalar
seed, the conformance harness deterministically tests it (a) as an explicit
fact, (b) as an inherited/defaulted fact, (c) wrapped in `prefix ` and ` suffix`
for substring rules, (d) with every UTF-8 byte encoded once as uppercase `%HH`,
and (e) with each `%` from transformation (d) encoded once more as `%25`.
Transformation (d) must decode once and match the canonical seed;
transformation (e) must decode once and reject because residual `%HH` escapes
remain. It is never decoded twice. The harness also represents ASCII punctuation
once as JSON `\u00HH` to verify matching after JSON decoding. Case-insensitive
rules are tested in lower, upper, and alternating case. Attribute-code seeds use
snake, kebab, and camel separators instead of scalar transformations. These
transformations and the table form the normative corpus.

| Outcome | Canonical seed | Rule |
| --- | --- | --- |
| reject | attribute code `apiKey` | normalized prohibited code |
| reject | `Jane@example.com` | email |
| reject | `+1 (415) 555-0123` | telephone with 11 digits |
| reject | `https://example.com` | URI |
| reject | `{550e8400-e29b-41d4-a716-446655440000}` | braced UUID |
| reject | `01ARZ3NDEKTSV4RRFFQ69G5FAV` | ULID |
| reject | `-----BEGIN PRIVATE KEY-----` | exact PEM marker |
| reject | `AKIAIOSFODNN7EXAMPLE` | AWS access key |
| reject | `QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFB` | 32-character base64, 24 decoded bytes |
| reject | `0123456789abcdef0123456789abcdef` | 32-character hex blob |
| reject | `2026-09-18T09:00:00Z` | RFC 3339 in a string attribute |
| allow | `Sample Navy Shirt` | no matcher |
| allow | `SKU-1001` | fewer than seven digits and no other matcher |
| allow | `abcdefabcdefabcdefabcdefabcdefa` | 31-character hex boundary |
| allow | `123 456` | six-digit telephone boundary |
| allow | typed date value `2026-09-18` on a date attribute | native typed date |

These checks are defense in depth, not semantic classification or provenance
proof. They cannot establish that an ordinary-looking name, description, or
number was not copied from production, and documentation, inspection, API, and
UI wording must never claim otherwise. Trusted distribution review and explicit
administrator approval remain required.

## Context and Visible Classification

Sample declarations cannot declare, create, map, update, or reference contexts.
At apply time the host omits `context_id` and resolves the workspace's ordinary
`default` context through the normal entity-create path. A sample that requires
any other context is blocked.

On creation the host injects the host-owned `attricat.sample` marker; archives
cannot supply it or any other system annotation. Entity detail, edit, search,
related, and incoming-preview API shapes expose a derived `is_sample` boolean,
and the frontend displays a clear **Sample** badge on list, relationship-picker,
detail, and edit surfaces.

The marker is visible classification, not ownership or immutable provenance.
After the application completes, an ordinary authorized edit may remove it.
Later retries or releases never restore the marker, and derived `is_sample`
reflects its current presence. Immutable application history remains the record
that the entity originated as sample data.

## Planning, Retry Identity, and Later Releases

A selected plan records the canonical sample declaration digest, explicit
selection, ordered logical keys and dependency edges, preallocated entity IDs,
exact blueprint IDs and revisions, canonical inputs, actions, preconditions,
and the automation warning. Its evidence digest covers all of those fields.
Omitting `--include-sample-data` records non-selection and produces no sample
mappings, actions, or mutations. Initial sample planning creates entities only;
it neither discovers nor maps arbitrary existing entities.

The exact same-release dataset identity is the tuple:

```text
(workspace ID, pack ID, pack SemVer, archive digest,
 canonical sample declaration digest)
```

The first successfully created sample-selected plan atomically and durably
reserves that identity. Every second independently created sample-selected plan
with the same tuple is rejected whether the first plan is not ready, ready, or
expired, or its application is running, failed, invalid, abandoned, or
completed. The reservation is never released or transferred. If the first plan expires before apply starts, that
exact dataset identity cannot be selected again; the administrator must use a
new pack release or archive with a different identity.

Same-release idempotency is solely retry/resume of the original immutable plan
and its one durable application. Repeated or concurrent apply calls with that
original plan ID lock and converge, completed steps are not rerun, and a retry
of a completed application is a no-op. This resumes all plan resources—not only
samples—through their recorded blueprint, setting, asset, sample, and other step
results, so there is no mixed-resource replay policy to infer.

`--from-application` remains prohibited for the same SemVer. It retains the
canonical solution-pack rule: an administrator may name exactly one completed
same-workspace application only while planning a strictly newer SemVer release
of the same pack. Catalog never searches history or infers lineage.

For that explicitly selected completed application of an older release of the
same pack:

- an added logical entity key is `create`;
- an unchanged canonical declaration reuses the exact recorded entity ID,
  without comparing current scalar values, relationships, or sample-marker
  state;
- a changed scalar, relationship, blueprint, or classification declaration
  blocks as `update_not_supported`; and
- a removed key is informational history only.

Existence and identity of a reused target are preconditions; a missing or
deleted target conflicts and is never recreated. Ordinary live-value drift is
not a conflict and is not repaired. No rule updates, replaces, deletes,
republishes, relabels, or migrates a reused entity.

## Private Values and Evidence Retention

This section governs solution-pack plan/application **staging copies**. It does
not shorten ordinary entity, audit, or outbox retention. Archive inspection may
parse canonical values transiently but retains and returns only safe bounded
summaries. Persisted staging values are available only to authorized plan/apply
paths and never appear in list responses, logs, or diagnostics.

Ordinary create behavior is intentionally unchanged: its entity values,
`audit_event_changes` before/after values, and `entity.created.v1` outbox facts
may contain the sample scalar values and remain under their ordinary retention
and access controls after solution-pack staging is scrubbed. Inspection and
plan warnings must say this explicitly. Staging cleanup is not an audit or event
redaction mechanism, and no contract text or UI may imply that all durable
copies of a created value are removed.

A sample plan uses the ordinary 24-hour plan lifetime. At `expires_at`, every
inspection, show, and apply path synchronously treats an unstarted plan as
inapplicable, refuses to read or return its staging payload, and cannot apply
it. A lock-safe sweeper runs at least hourly and physically deletes the raw scalar and
relationship staging rows no later than one hour after expiry. Until that purge
finishes, expired staging rows are inaccessible through product paths and are
not part of the retained plan response.

Starting apply transfers the staging payload to the durable application under
the application lock and assigns a fixed `resumable_until` of 30 days after
`started_at`; it is not extended by retries. Plan expiry then cannot strand the
application. A transient or ambiguous failure remains nonterminal and retains
the values only until that deadline so retry and commit reconciliation can
resume without re-uploading or changing reviewed input. Earlier successful
creates are not rolled back or deleted. No step may begin at or after
`resumable_until`, and sample step transactions use a maximum five-minute lock
and statement timeout.

An authorized administrator may abandon a resumable application earlier with
`acli solution-pack applications abandon <application-id>` (or the permissioned
`POST /solution-packs/applications/{application_id}/abandon` API). At
`resumable_until`, the hourly API housekeeping worker abandons it no later than one hour after the
deadline. Manual and automatic abandonment acquire the same application lock as
apply and wait, for at most the step timeout, for an active step transaction.
They reconcile the exact preallocated entity ID and durable step result: a
matching committed step remains completed, an absent target remains unexecuted,
and mismatched target/evidence becomes a bounded permanent-conflict result.
They then mark remaining steps unexecuted, transition to terminal `abandoned`,
and atomically scrub all staging values. Reconciliation never adopts, changes,
or deletes an entity and cannot extend the purge deadline.

Transitions to `completed`, permanently `invalid`, or `abandoned` atomically
remove every staging scalar and relationship input after all step results needed
for reconciliation are durable. A state called `failed` is nonterminal only
before `resumable_until`; it retains inputs until retry, explicit abandonment,
or automatic abandonment. A terminal transition cannot commit while staging
values remain attached to that plan/application.

Expired and terminal solution-pack records retain only archive,
whole-declaration, and per-entity canonical declaration digests; logical keys;
bounded non-value summaries and counts; entity/blueprint mappings; actions and
results; actor/request/correlation evidence; diagnostics; and timestamps. They
do not retain source JSON, canonical raw scalar values, or raw relationship
inputs. Abandonment never deletes or modifies entities created by completed
steps.

## Application Semantics and Non-goals

Each create uses its preallocated ID, ordinary default-context resolution,
defaults, scalar/schema/relationship/cardinality validation, and the ordinary
entity transaction. The entity, `attricat.sample` marker, ordinary audit,
`entity.created.v1` outbox event, and solution-pack step result commit
atomically. Completed steps are never rerun. Ambiguous commits reconcile the
exact preallocated entity ID and recorded step before continuing.

Application is resumable, not a dataset-wide rollback transaction. A later
failure leaves earlier completed entities as ordinary workspace state. There is
no pack operation to update or delete them, no failure cleanup that destroys
entities, and no uninstall.

This contract explicitly does not provide:

- sample updates, replacement, migration, deletion, rollback, reset, export, or
  pack-level uninstall;
- ownership, drift reconciliation, marker restoration, or continuing lifecycle
  control;
- entity or publication-channel publication behavior;
- arbitrary existing-entity discovery, adoption, mapping, or content-based
  deduplication;
- context selection or mapping, files, remote downloads, cross-pack references,
  or cyclic relationships; or
- suppression of ordinary audit, workflow, extension, or domain-event behavior.

## Verification boundaries

The implementation's tests cover these boundaries:

1. strict schema, bounds, digest, and synthetic attestation fail closed, and
   the version-1 matcher passes every canonical, JSON-escaped, single-percent,
   residual-double-percent, PEM marker/envelope, explicit, inherited, and
   defaulted corpus case without overrides or a second decode;
2. sample selection is absent by default and requires
   `--include-sample-data`, with prominent inspection and plan warnings about
   automation and ordinary audit/outbox value retention;
3. context references, UUID/existing-entity references, and cyclic or
   cross-file relationships are rejected, while DAG creation is deterministic;
4. create uses ordinary validation, default-context resolution, audit, and
   `entity.created.v1` dispatch atomically with application-step evidence, and
   ordinary audit/outbox copies survive staging cleanup;
5. APIs derive `is_sample` from the host marker and every required frontend
   surface displays **Sample**; ordinary removal is allowed and never repaired;
6. the durable uniqueness reservation rejects every second independently
   created sample-selected plan for an exact dataset identity in every first-plan
   state; only the original plan/application can retry or resume, including all
   non-sample steps, and same-release `--from-application` is rejected;
7. later-release added/unchanged/changed/removed cases produce respectively
   create/reuse/`update_not_supported`/informational results without live-value
   drift checks; and
8. tests at plan expiry, the one-hour purge deadline, retryable failure,
   ambiguous commit, manual abandonment, the 30-day resumability deadline,
   completion, and permanent invalidation prove lock-safe access revocation and
   staging cleanup without deleting completed entities or needed retry evidence.

## Consequences

This contract preserves ordinary entity behavior and makes duplicate prevention
and retention testable. It also intentionally rejects useful-looking ambiguous
values, allows enabled automation to react to sample creates, and can leave a
partially applied dataset after a permanent failure. Those costs are accepted
instead of claiming unverifiable provenance, inventing sample-only event
semantics, or adding pack ownership and destructive cleanup.
