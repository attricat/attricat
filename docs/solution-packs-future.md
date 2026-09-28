# Solution-pack design ideas (not implemented)

This page records possible future behavior, **not accepted manifest syntax or
available CLI/API features**. For the shipped contract, use [Solution packs](solution-packs.md).
Do not author a pack against the proposals below.

## Installer options and prerequisites

A future installer could declare optional components and dependencies through
logical keys. It would need to reject cycles and omissions of transitive
requirements. Changing options after application would require a new plan, not
an in-place toggle.

A prerequisite pack could be identified by immutable pack ID and SemVer range,
validated against completed application records; it must not silently install
another pack or create continuing ownership. Missing or conflicting
prerequisites would block a plan. Security-sensitive choices such as extension
grants, enabling, destructive changes, and sample-data import must never be
selected solely because the pack author marks them as defaults.

## Successor and update planning

Current explicit-lineage planning reuses only unchanged targets; it never
updates a resource or creates a successor blueprint. Any future planner that
proposes compatible updates or draft successors must preserve these boundaries:

- Published blueprints are not edited in place.
- Workspace modifications are not silently reset to a pack baseline.
- Extension upgrades, grants, configuration, and enabling retain their ordinary
  lifecycle and approvals.
- Workspace settings preserve unrelated keys and user additions.
- Removal of a key in a later release does not delete the workspace resource.

## Export to draft

A possible exporter would produce an **untrusted draft**, never a trusted pack
release or raw workspace dump. It would replace workspace IDs and physical
references with logical keys, report references that cannot be made portable,
and require its output to pass normal archive validation. It must exclude
secrets, grants, users, memberships, audit data, runtime state, object keys,
customer data, and sample entities. Export would not confer permission to
redistribute third-party assets or extension packages.
