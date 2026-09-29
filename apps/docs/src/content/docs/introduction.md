---
title: Introduction
description: What Attricat is, what it is for, and how its parts fit together.
---

Attricat is a catalog for structured product and reference data. You describe your records with versioned **blueprints**, keep values that differ by market, language, or channel in **contexts**, link records with typed **relationships**, and approve what goes out to each channel by **publishing** it.

Every change is validated on the server, recorded in the audit log, and kept in history. Integrations, workflows, and AI agents change data through the same path as people, so the rules hold no matter who makes the change.

## What it is good at

- **Catalogs whose shape changes.** A blueprint revision never changes after it is published, and each entity stays on the revision it was written under until you migrate it. You can evolve the model without breaking what is already there.
- **Values that differ by place.** A context tree (for example market, then channel) lets each value be set once and inherited, with overrides only where they are needed.
- **Classifications and taxonomies.** Categories, brands, and materials are entities of their own, linked by relationships, so they can be renamed, translated, and arranged in hierarchies.
- **Controlled output.** Publishing approves an entity for a channel. Any later edit withdraws the approval until someone publishes again.
- **Extending safely.** Extensions run in sandboxes and can only do what an administrator grants them.

## How the parts fit

| Part | What it is |
| --- | --- |
| **Workspace** | One catalog with its own members, data, and settings. |
| **Blueprint** | A versioned TOML definition of a record type: attributes, validation, and layout. |
| **Entity** | One record, pinned to a blueprint revision. |
| **Attribute** | A field of an entity: text, number, date, relationship, file, and more. |
| **Context** | A place where values can differ, arranged in a tree under `default`. |
| **Relationship** | A typed link from one entity to others. |
| **Publication channel** | A context enabled for export, where entities are approved one by one. |
| **Rule** | A data-quality check that records findings. |
| **Workflow** | A small automation that tags or updates an entity after an event. |
| **Extension** | A package adding server logic, UI, attribute types, or integrations. |
| **Solution pack** | An archive that sets up a workspace for a use case. |
| **Agent** | An AI assistant that reads the catalog and proposes changes for approval. |

[Core concepts](/start/concepts/) explains each of these in more depth.

## Where to go next

- New to Attricat? Follow the [Quickstart](/start/quickstart/) to build a small catalog.
- Designing a catalog? Read [Model your catalog](/builders/modeling/) and [Author a blueprint](/builders/blueprints/).
- Running Attricat? Start with [Deploy Attricat](/operate/deployment/) and the [configuration reference](/reference/configuration/).
- Building an integration? See the [API reference](/reference/api/), the [CLI](/reference/cli/), and [Build an extension](/extensions/build/).
