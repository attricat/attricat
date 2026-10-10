# Caching

The API caches a small set of database reads that hot request and worker paths
repeat: published blueprint revisions, the context tree, enabled rules,
extension manifests and client contributions. This page describes how the
cache stays correct and what a new write path must do to keep it that way.

HTTP caching of downloads is a separate concern; see the
[API reference](api.md#file-uploads-and-downloads).

## The `catalog-cache` crate

`crates/cache` (`catalog-cache`) is a leaf crate with no catalog knowledge. The
repository decides what is cached and under which key; the crate provides:

- `QueryCache`: a bounded in-process LRU (L1, Moka, `CACHE_MAX_ENTRIES`
  entries) and an optional shared tier (L2, Redis). An L1 hit is an `Arc`
  clone with no serialization. Concurrent misses for one key run the loader
  once; a failed load is never cached.
- `RateLimiter`: the sliding-window limit on extension network requests,
  shared through Redis when it is configured.
- `CacheConfig`: `CACHE_BACKEND`, `REDIS_URL`, `CACHE_MAX_ENTRIES` and
  `CACHE_KEY_PREFIX` (see [Configuration](configuration.md)).

`apps/api` builds one `QueryCache` at startup and hands it to every request and
worker repository with `CatalogRepository::with_cache`. Repositories built
without it (tests, maintenance) get a private in-memory cache.

Keys are `namespace:part:part`; the namespace (`blueprint_revision`,
`context_tree`, `enabled_rules`, `extension_manifest`, ...) labels the
`catalog_query_cache_requests_total` metric.

## Correctness comes from keys, not invalidation

Nothing is invalidated across replicas, and there is no invalidation bus.
Every entry is correct because of how its key is built. Each entry has one of
three policies:

| Policy | Meaning | Used for |
| --- | --- | --- |
| `Immutable` | The value never changes for its key; only LRU eviction removes it. | Published blueprint revisions (`blueprint_revision`, `blueprint_revision_code`) and installed release manifests (`extension_manifest`). Drafts are never cached. |
| `Generation` | The key embeds a workspace generation the request read, so a write makes a new key and old entries age out unused. | `context_tree`, `enabled_rules`, `blueprint_latest_id`, `blueprint_latest_code`, `client_contributions`, `extension_layout`. |
| `Ttl { fresh, stale }` | Served for `fresh`; for a further `stale` one caller reloads while others get the stale value. Use it only where bounded staleness is acceptable. | `active_workspaces` (5 s fresh, 5 s stale), the background loops' workspace list. |

`Tag`s and `QueryCache::invalidate_local` evict L1 entries of one process only.
They never reach Redis or other replicas, so they must never be what keeps an
entry correct.

The data-health endpoints use their own response cache in `catalog-http`,
keyed by the workspace outbox high-water mark and bounded by
`DATA_HEALTH_CACHE_TTL_SECONDS`.

## Workspace generations

`workspaces` has three counters (see [Database model](database.md#workspaces)):

| Generation | Covers | Advanced by |
| --- | --- | --- |
| `catalog_generation` | Latest published revisions, enabled rules, Explore navigation | Publishing a blueprint revision, publishing, enabling or disabling a rule, writing Explore navigation |
| `contexts_generation` | The context tree | Creating, updating or deleting an attribute context |
| `extensions_generation` | Client contributions and extension layout | Every installation, upgrade, enable, quarantine, removal, grant, revocation and configuration change (all commit through `commit_extension_mutation`), lifecycle state changes, layout writes and the workspace extensions mode |

The write transaction advances the counter with
`generations::advance_generation` before it commits. A reader reads the
counters in a query it already runs (session or token authentication, the
record lock of a write) and only then looks up cached state under that
generation. A reader can therefore never use state older than the generation it
read, and a committed write is visible to the very next request on every
replica.

Record writes prefetch the context tree and enabled rules from committed data
before their transaction opens (`WritePrefetch`). The transaction uses the
prefetch only if the generations its own record lock reads are the same, which
also rules out uncommitted changes of its own.

## Transactions consult the cache but never fill it

A transaction may read a cached entry, because every entry describes committed
data. It must not store what it reads: it could see its own uncommitted change,
such as a publication that is later rolled back, and cache it for everyone.
Values are cached by committed reads on the pool. The one exception is a row
that is provably committed before the transaction could see it, such as an
installed release, which only its own install transaction inserts
(`cached_release_manifest_on`).

## Adding a cached read or a write path

Any new write path that changes cached data must advance the right generation
in the same transaction. Forgetting it serves stale data until the process
restarts or the entry is evicted, on every replica. When you add one:

1. Find every cached read whose loader reads the tables you write, and advance
   its generation with `advance_generation` inside your transaction.
2. Add a test like `apps/api/tests/generation_caches.rs`: change the state and
   assert the very next request sees it.

When you add a cached read:

1. Prefer `Immutable` for data that never changes for its key, and
   `Generation` with the narrowest existing counter otherwise. A new counter
   needs a migration and must be read by authentication with the others.
2. Load from the pool, never from a caller's transaction.
3. Make every value `Serialize + Deserialize`; it may be shared through Redis.
4. Bump `CACHE_FORMAT` in `crates/cache/src/lib.rs` when you change the
   serialized shape of a cached value, or what a loader returns for an
   existing key. The version is part of every Redis cache key
   (`cache:v<N>:`), so the new release never reads entries the previous one
   wrote; otherwise Redis could serve them for up to 24 hours after a deploy.
   A new key namespace does not need a bump.

## Shared tier (Redis)

With `CACHE_BACKEND=redis`, values are also stored in Redis as JSON so replicas
share loads. Redis is an optimization, never a dependency:

- Every command has a 250 ms deadline. Errors degrade to L1 and the database
  and never fail a request.
- Writes to Redis run in the background, so a slow Redis never delays the
  request that loaded the value.
- After 5 consecutive timeouts or connection failures Redis is skipped for
  5 s (a circuit breaker shared by the cache and the rate limiter). After the
  cooldown one command tries Redis while the others keep skipping it; if it
  succeeds the breaker closes, otherwise another cooldown starts. An error
  Redis returns for a command, such as `NOPERM`, is logged but does not count.
- While the client is disconnected, commands skip Redis without waiting. The
  first disconnect is logged at `warn` and the reconnection at `info`.
- The client connects in the background and reconnects with a capped
  exponential backoff (100 ms to 5 s), including when Redis is unreachable at
  startup.
- `Immutable` and `Generation` entries expire from Redis after 24 hours of the
  last write; a `Ttl` entry after `fresh + stale`. A `Ttl` entry copied from
  Redis into L1 keeps its age, so it never outlives its bound.
- TLS URLs (`rediss://` and `valkeys://`, and their `-cluster` and
  `-sentinel` forms) use rustls with the ring provider and the platform root
  certificates.

Every Redis key starts with `<CACHE_KEY_PREFIX>:<database identity>:` followed
by `cache:v<CACHE_FORMAT>:` or `rate:`. The database identity is a random UUID
that the API stores in the single-row `database_identity` table at startup.
Separately created databases that share one Redis therefore never read each
other's entries, even though generations restart at 0 and the bootstrap
workspace ID is fixed.

A copy of a database keeps its identity and its generations. That includes a
restored backup and a clone such as a staging database copied from
production, so both can find entries filed under the same keys with other
content:

- **A copy that runs alongside its source** (staging cloned from production,
  a restore rehearsal) must use a different `CACHE_KEY_PREFIX` or a different
  Redis database (the number in `REDIS_URL`) than the source. Flushing Redis
  does not help: the source writes the shared keys again.
- **A restore in place**, which replaces the database a Redis served: flush
  that Redis database (`FLUSHDB`) or change `CACHE_KEY_PREFIX` before starting
  the API.

See [Deployment](../apps/docs/src/content/docs/operate/deployment.md).

Extension network rate limits use the same sliding window in Redis (a sorted
set updated atomically by a Lua script, using the Redis server clock) as in
process memory. While Redis is unavailable each replica falls back to its own
limiter, so the limit applies per replica until Redis recovers.

## Metrics

- `catalog_query_cache_requests_total{namespace,outcome}`: `outcome` is `hit`,
  `remote_hit`, `miss`, `stale` or `stale_reload`.
- `catalog_query_cache_invalidations_total`: process-local tag evictions.
- `catalog_query_cache_redis_connected`: `1` while the Redis connection is
  up, `0` while the client is disconnected and reconnecting. Each process
  reports its own connection.
- `catalog_query_cache_redis_circuit_opened_total`: times the circuit breaker
  opened after repeated timeouts or connection failures. It counts openings,
  not skipped commands; a failed trial after the cooldown does not count
  again. A disconnect alone does not open the breaker, so watch the gauge for
  outages.

See [Metrics and traces](api.md#metrics-and-traces) for the rest, including the
database round-trip metrics the cache is meant to reduce.
