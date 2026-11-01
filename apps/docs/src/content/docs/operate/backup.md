---
title: Backup and restore
description: Back up the database and object storage together, and rehearse restoring them.
---

An Attricat catalog lives in two places: PostgreSQL holds the data and file metadata, and the S3 bucket holds the files, image variants, extension packages, operation outputs, and presentation assets. The two refer to each other, so **back them up and restore them together**. Restoring only one leaves records without files, or files nobody can reach.

The Attricat image contains no `pg_dump`, `pg_restore`, or S3 tools. Use version-matched tools from your platform or a separate administration container.

## Take a backup

1. Stop writes: stop the API and the file worker, at a point you can name. Pausing traffic isn't enough, because the API also runs extension operations and other background work.
2. Record the Attricat image digest and the time.
3. Dump the database with a PostgreSQL 18-compatible `pg_dump --format=custom`.
4. Snapshot the whole bucket while writes are still stopped.
5. Write a manifest that pairs the dump and the snapshot, with checksums and your provider's snapshot or version IDs. For a plain bucket copy, per-object checksums are enough.
6. Resume only after both halves and the manifest are stored safely.

If your provider can take database and bucket snapshots at the same stopped point, use them instead.

Keep credentials in your secret manager, not in commands, logs, or the manifest.

## Rehearse a restore

Rehearse at least once per release, into a disposable database and bucket:

1. Keep the target stopped.
2. Verify every checksum in the manifest.
3. Restore the database and the bucket. For a different database role, use `pg_restore --no-owner --no-privileges --exit-on-error`. A bucket can have a new name: set `S3_BUCKET` to it.
4. If the deployment uses Redis (`CACHE_BACKEND=redis`), give the restored copy a different `CACHE_KEY_PREFIX` or Redis database than the deployment it was copied from. A restored database keeps its cache identity, so otherwise the two share cache entries and the API may serve data newer than the restored database for up to a day. Flushing Redis (`FLUSHDB`) is enough only when you restore in place, replacing the database that Redis served; while the source keeps running, it writes the same entries again.
5. Run the image's `migrate` role.
6. Start the API and file worker against the restored pair.
7. Wait for both readiness checks. They don't look at the bucket's contents: an empty bucket still reports ready, so only the next step shows the bucket was restored.
8. Check a known record, a known uploaded file and its image variant, a presentation asset, and a known extension operation output, then run an extension operation to prove its package was restored. Outputs expire after 30 days, so an older backup may have none.

A rehearsal is a working copy of your deployment. Give it no real mail relay, and turn off exports and extension connections, so it can't reach live systems.

If any step fails, leave the target stopped and investigate. Never go live with only one half restored.

## Retention to be aware of

- **Value history** older than `ATTRIBUTE_VALUE_HISTORY_RETENTION_DAYS` (90 by default) is deleted when the API starts.
- **Unreferenced files** are marked deleted, then purged after `FILE_DELETE_GRACE_SECONDS` (one day by default). To keep such a file, recreate its reference before the purge runs. Don't rely on the grace period as a backup.
- **Extension operation outputs** are kept for 30 days after the run completes.
- **Events and audit records** are not pruned automatically. Do not delete them by hand; the event system depends on them.
