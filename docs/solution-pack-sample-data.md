# Using optional solution-pack sample data

Packs may include fictional records for trying out a solution. Samples are off
by default; an administrator must select them when creating a plan. See
[installing a pack](solution-packs.md) for the full workflow.

## Review before opting in

Test samples in a disposable development workspace. Only select data from a
trusted, reviewed publisher. Server validation and the publisher's declaration
that the data is fictional cannot prove that it is safe or was never copied
from production.

Sample creation uses ordinary record validation, permissions, audit, and
`record.created.v1` events. Enabled workflows or extensions may run and
cause external effects. Inspect the workspace's automation before applying.
Values can remain in ordinary record, audit, and event history even after the
pack's temporary staging copies are removed.

## Select samples during planning

```sh
acli solution-pack inspect --file pack.tar.zst
acli solution-pack plan --file pack.tar.zst --prefix example \
  --blueprint-publication publish --include-sample-data
acli solution-pack plan show <plan-id>
```

Inspection reports sample counts and warnings without creating records. Review
these warnings, the plan's selected-sample flag, actions, and readiness. Omit
`--include-sample-data` to install the pack without creating samples.

Apply takes only the reviewed plan ID; there is no sample-data switch at apply:

```sh
acli solution-pack apply <plan-id>
acli solution-pack applications show <application-id>
```

Samples are created against the plan's exact published blueprints. Values are
set in the workspace's default context unless the pack sets them in one of its
own contexts; those values use the context the plan creates or that you map
with `--map-context` (see [contexts](solution-packs.md#contexts-and-publication-channels)).
A sample that needs a context the plan cannot create or map stops planning.

Samples cannot set status or principal (user or team) attribute values: status
values follow the workspace's transitions and principals name workspace
members, so a pack that sets them is rejected during inspection. Status
attributes take their ordinary defaults. Samples of one blueprint also cannot
share a value of one of its unique keys; inspection compares them the way the
workspace does (trimmed, whitespace collapsed, and case-insensitive unless the
key is case-sensitive). An existing record of a mapped or reused blueprint can
still hold the same key, and then the sample's step fails on apply.

Samples can attach files bundled in the archive, such as images, PDFs, or plain
text documents. Planning uploads each bundled file once to ordinary file storage
before saving the plan; apply attaches it to the sample record as an ordinary
file in the same transaction that creates the record, and validates the record
again with its files before recording the ordinary audit record and event. The
file then shows its usual processing status. Inspection reports the
number of bundled files (`sample_data.file_count`). Attricat checks each file's
type and the attribute's file policy, but cannot inspect file content for
copied or private data; review it with the publisher.

## Recognize and work with samples

Created sample records have a visible **Sample** badge. Users with the required
permissions can edit them, including removing the sample marker. The pack does
not own them. Application history still records their origin, and later retries
do not restore removed markers or reset user edits.

Inspect the resulting relationships and follow the publisher's walkthrough.
Removing the sample marker does not remove values from audit/event history.

## Retry, expiry, and abandonment

The first plan with samples selected reserves that exact combination of release,
archive, and dataset in the workspace. A second plan cannot select the
same identity, even if the first plan expires or its application fails or is
abandoned. Changing the prefix does not reset this reservation.

- Retry or resume using the original plan ID. Reapplying a completed plan does
  nothing; it does not create the sample records again.
- Start within the plan's 24-hour lifetime. If it expires before application
  starts, coordinate a new release with the publisher; do not expect a fresh
  plan of the same dataset to work.
- Once a sample-selected application starts, it has a fixed 30-day resumability
  window. Retries do not extend it.
- To permanently stop a resumable sample-selected application, use:

```sh
acli solution-pack applications abandon <application-id>
```

Abandoning an application leaves previously created resources, records, and the
dataset reservation in place. The host also abandons these applications after
their resumability deadline.

Temporary staged sample values are scrubbed on completion, permanent invalidation,
or abandonment. Unstarted expired plans lose access to staged values at expiry,
and housekeeping purges those copies within one hour. Bundled files that were
uploaded but never attached are released at the same points and removed by
ordinary upload cleanup. Ordinary record, audit, and event records remain
unchanged.

## Samples in later releases

For a strictly newer release, an administrator may select one completed earlier
application with `--from-application`. New sample keys can create records;
unchanged declarations reuse their recorded record identities without resetting
live values or sample markers. Changed declarations are blocked, and removed
ones do not delete records. Missing reused records cause conflicts rather than
automatic recreation.

## Cleanup limits

There is no pack operation to reset, update, publish, delete, or uninstall a
sample dataset. Use ordinary authorized record/resource operations when cleanup
is appropriate, and respect relationships, business data, and retention policy.
Deleting a sample record through ordinary operations removes it from current
reads together with its values in every context and its attached files, like
any other record. Attached files are ordinary files and follow ordinary file
retention: a stored file is removed only once no attribute value, including
value history, references it. Contexts the pack created stay in place until you
delete them.
An interrupted application can leave completed steps in place; there is no
whole-dataset rollback.
