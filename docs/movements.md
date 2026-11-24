# Balances and Movements

A `balance: {min}` attribute is a running total that only movements change,
such as a stock level. A record blueprint's `movements` (at most
`MAX_MOVEMENTS`, 4) say how creating one of its records changes the balance of
a record of another blueprint, found through one of that blueprint's unique
keys.

```yaml
movements:
  - code: stock
    direction: out            # in | out (MovementDirection)
    quantity: quantity        # number/integer, > 0
    factor: {relationship: conversion, attribute: factor}   # optional
    result: base_quantity     # optional; required with reversal
    target:
      relationship: stock_item
      blueprint: stock_item
      balance: quantity
      key: product            # a workspace-scoped unique key of stock_item
      values: {product: product}
      create_missing: false
    reversal: {relationship: reverses}   # optional
```

Contract and checks:

- `crates/blueprint/src/movements.rs` holds `BalancePolicy`,
  `MovementDefinition` and `TargetScope`. The parser checks structure (record
  blueprints only, codes, every named attribute declared, one role per
  attribute); the compiler checks types against the effective attributes: the
  quantity is a number, the factor, target and reversal links are
  single-target relationships, the target link targets exactly
  `target.blueprint` and the reversal link this blueprint, and every attribute
  a movement reads is `context_editable: default`. The target link and the
  result are compiled read-only and default-context only, as balances are.
- Publication (`validate_publication_movements`) checks the target and factor
  blueprints at their latest published revisions: the balance attribute, a
  workspace-scoped key whose attributes are exactly `target.values`' keys with
  matching types and targets, and a number factor. Publish targets first;
  solution packs rewrite `target.blueprint` like relationship targets and order
  a movement's target and factor blueprints before it (`movement_dependencies`).
- Storage: `attributes.balance`, `attributes.target_scope` and
  `blueprints.movements` (JSONB, migration
  `20261109035973_balances_movements_and_target_scopes`).

Write semantics (`crates/repository/src/repository/movements.rs`), inside
`create_record_in_transaction`, which every create path shares:

1. Before the record is inserted, `prepare_movements_in` reads the submitted
   default-context quantity and factor link, computes `quantity × factor` in
   `rust_decimal::Decimal`, takes the target key's advisory lock and locks the
   target row found through `record_unique_key_values`, or creates the target
   through the create seam (`create_missing`; a sample record's target is a
   sample too). This keeps target rows before the record's counters and key
   locks; see the lock order in `repository/mod.rs`.
2. The record is written with the target link and the result, with
   `ValueOrigin::Movement`.
3. After the record validates, `apply_movements_in` changes each balance
   through `update_record_from_movement_in` (the ordinary update seam with
   `ValueOrigin::Movement`), so status locks, audit and `record.updated.v1`
   apply. The audit event targets the target record with action
   `attricat.records.movement.balance_update` and `metadata.movement`. A result
   below `min` fails with `409 balance_below_minimum` and rolls the whole
   create back.

A reversal (a record that sets `reversal.relationship`) locks the original and
its target in ID order, refuses a reversal of a reversal and a second
reversal (checked under the original's row lock), moves the original's
`result` the other way and copies the original's inputs; submitted inputs must
match (`movement_reversal_mismatch`).

Afterwards the values a movement read or set never change:
`ensure_movement_values_unchanged`, called from `validate_record_schema_in`,
compares their default-context values before and after every write except the
record's own create (`WriteContext::new_record`) and fails with
`409 movement_immutable`. The same check keeps the attributes of a unique key
that some published movement finds its target by (`WriteAttribute::movement_key`):
once such a key value is set it cannot change (`409 movement_target_key_immutable`). `WriteAttribute::ensure_client_writable` rejects
client writes of balances (`balance_attribute_readonly`) and of the values a
movement sets (`movement_attribute_readonly`) on every value and relationship
path. Deleting a movement record fails with `movement_record_undeletable`, and
deleting a record with a non-zero balance with `balance_record_not_empty`;
sample records are exempt. A duplicate leaves out every core-assigned value
(`core_assigned_attribute_codes`) and applies its own movement.

Numbers cross the API as JSON (f64) numbers, while values, the arithmetic and
error details are exact decimals; `error.details` carries decimal strings.

## Applied movements

Every movement applies to its record at most once. Each application is a row
of `record_movement_applications` (migration
`20261109035974_record_movement_applications`): the record, the movement code,
the target, the balance, the amount and the direction. The record form
response lists the record's `applied_movements`, and an order's form its
`settled_lines`. The rules above read those
rows rather than the blueprint:

- `ensure_movement_values_unchanged` guards the inputs and managed values of
  the movements that applied before this transaction (`applied_at < now()`),
  so a draft stays editable until its movements apply.
- Deletion is refused once any movement applied (`movement_record_undeletable`);
  a record with nothing applied may be deleted.
- A reversal moves back every applied movement of the original, latest first
  and in reverse declaration order within one save, on the recorded targets
  and amounts. Reversing a record with nothing applied fails with
  `409 movement_not_applied`. A reversal applies on create; its own status
  never applies its movements again.

## Movement history

`sequence` numbers a target's applications from 1, without gaps (migration
`20261109035975_record_movement_application_sequence`, unique per target).
`apply_movements_in` assigns `MAX(sequence) + 1` after the balance change,
which holds the target's row lock until commit, so the sequence is the order
the target's balances changed in. `applied_at` is the transaction's start and
can disagree when two saves wait on one target.

`movement_ledger` pages a target's applications by sequence (at most
`MAX_MOVEMENT_LEDGER_PAGE`, 500) with each record's blueprint code and the
amount as a decimal string. For an authorization actor it leaves out the
record, its blueprint and movement code of each entry whose record the actor
may not read, keeping the entry, so the history stays gapless. It backs
`GET /v1/records/{id}/movements` (`records.read` on the target, the caller as
actor), `acli record movements` and the extension
read `{"operation": "movements"}` of `attricat.read.v1`. Replaying a target's
history rebuilds its balances; derived views such as stock valuation
replay it rather than keep their own copy. There is no separate event: every
application updates the target, which publishes `record.updated.v1`.

## Status-triggered movements

`apply_on` lists status transitions that apply a movement instead of the
create; `requires` names earlier-declared movements that must have applied
first, or the movement is skipped. `MAX_MOVEMENT_TRIGGERS` is 8.

```yaml
movements:
  - code: reserve
    direction: in
    quantity: quantity
    target: {relationship: stock_item, blueprint: stock_item, balance: reserved,
             key: item, values: {item: item}}
    apply_on: [{attribute: status, to: approved}]
  - code: release
    direction: out
    quantity: quantity
    target: {relationship: stock_item, blueprint: stock_item, balance: reserved,
             key: item, values: {item: item}}
    apply_on: [{attribute: status, to: completed}, {attribute: status, from: approved, to: cancelled}]
    requires: [reserve]
  - code: consume
    direction: out
    quantity: quantity
    target: {relationship: stock_item, blueprint: stock_item, balance: quantity,
             key: item, values: {item: item}}
    apply_on: [{attribute: status, to: completed}]
    requires: [reserve]
```

- The compiler checks that `attribute` is a status attribute of the blueprint
  and that `to` and `from` are its options. A trigger with `relationship` is
  checked at publication against the linked blueprint's status.
- Movements may share a target link when they find the same record (same
  blueprint, key, values and `create_missing`); core writes it once.
- `validate_record_schema_in` calls `apply_status_movements_in` with the
  save's default-context `StatusChange`s, so every write path (forms,
  batches, workflows, agents, extensions) applies them in the save's
  transaction. Matching movements apply in declaration order, skipping those
  already applied or whose `requires` have not applied. A failure, such as a
  shortage, fails the whole save and leaves the status unchanged.
- Lock order: before the update seam locks the record, `pre_lock_movement_rows`
  (only for client writes of a status attribute some movement triggers on)
  takes the relationship lock and locks the record, the existing targets of
  its triggered movements and its lines with theirs, in ID order.

## Reservations

`balance: {min, reserved}` names another balance of the same record that holds
part of this one. `min` then applies to the balance minus the reserved one,
after a change to either: an issue cannot take reserved stock, and a
reservation cannot hold more than is free. `balance_below_minimum` details
then report the free amount as `available` and add `reserved`. The compiler
requires `reserved` to name another balance without a reservation of its own
(`validate_reserved_balances`).

## Line triggers

A trigger with `relationship` applies a movement of a *line* record when the
record it links to makes the transition, such as the material lines of a
production order:

```yaml
apply_on: [{relationship: production, attribute: status, to: approved}]
```

The relationship must be single-target and `context_editable: default`.
`apply_line_movements_in` finds the published blueprints whose triggers link
to the saved record's blueprint, then the lines linking to it in ID order, at
most `MAX_MOVEMENT_LINES` (200; more fails with `422 movement_lines_exceeded`).
Each line is locked, its movements apply as above and the line is updated with
audit action `attricat.records.movement.line_apply`. One failing line fails
the whole transition. Once a line's movement applied, the order's lines of that
blueprint are settled: adding, moving or deleting one fails with
`409 movement_lines_locked` (`ensure_movement_lines_open`). Lines of another
blueprint settle with their own movements, so output lines stay open until
completion.
