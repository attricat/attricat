---
title: Explore entities
description: Find, inspect, and maintain catalog records.
---

Open **Entity explorer** to browse the entities available in your workspace.

## Find records

Choose a blueprint to start exploring its entities. Use the search field to match display values and attributes. Queries can target a field, for example `sku:123*`, or a nested field, for example `color.name:red`.

Use the context selector to see values as they resolve in a particular context. A value can come directly from that context or be inherited from its default value.

## Work with an entity

Open a result to inspect its values, schema revision, files, relationships, and change history. If you can edit the entity, save changes from its detail page. Read-only fields remain visible but are managed by a system integration rather than the browser.

When the entity's blueprint has a newer published revision, Attricat identifies the record as outdated. Review the migration before upgrading so changes to fields or validation are deliberate.

See [Contexts](/guides/contexts/) for how inherited values work.
