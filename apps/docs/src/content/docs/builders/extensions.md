---
title: Extensions
description: Add trusted integrations and client contributions to Attricat.
---

Extensions add capabilities to Attricat without changing the core catalog. They can contribute workspace-scoped configuration, background behavior, events, and browser UI.

## Install from a trusted source

Open **Manage → Extensions** to browse registry entries or upload a local extension package when you have the required permission. Attricat validates packages before installation and installs them disabled.

## Review before enabling

An extension requests permissions for the host functions, events, storage, and client contributions it needs. Review those requests, configure the extension, grant only the required permissions, then enable it.

An enabled extension is trusted workspace software. Keep its permissions narrow and use an extension source you trust.

## Build an extension

Extension authors package a strict manifest and target a supported Catalog host API contract. Test against a local Attricat workspace before publishing a release.

The public extension authoring reference is the next documentation section to expand; until then, use the repository's extension contract as the implementation source of truth.
