---
title: Compatibility
description: What Attricat keeps stable between releases, what can change, and how removals are announced.
---

Attricat follows [Semantic Versioning](https://semver.org/) from 1.0. A patch or minor release doesn't break anything listed as stable here. A breaking change needs a new major version.

## Stable

- **HTTP API:** the routes, request fields, and response fields in the [API reference](/reference/api/). New optional request fields and new response fields can appear in any release, so clients must ignore fields they don't recognize.
- **Error codes:** the `code` of an API error. The English `message` can change, so match on the code.
- **Blueprint format:** a blueprint that is valid today stays valid.
- **CLI:** the commands and flags in the [CLI reference](/reference/cli/), and the fields of their JSON output.
- **Configuration:** the [settings and secrets](/reference/configuration/) operators set.
- **Events:** the [event envelope](/reference/events/).
- **Extension host ABI:** released `attricat:host` versions only grow. See [host ABI versions](/extensions/server/).

## Not stable

- The database schema. Read and write through the API or `acli`. Back the database up, but don't build on its tables.
- The web app's layout, wording, and internal requests.
- Metric names and labels, and log messages.
- The content of any response marked internal, and of routes missing from the API reference.

## Upgrades

Run migrations before rolling out a new version, and take a backup first. A previous version isn't guaranteed to run on a migrated database, so a rollback can mean restoring from that backup. See [Deployment](/operate/deployment/).

A release that needs something from operators says so in its notes.

## Deprecation

Something stable is deprecated in a minor release and stays working until the next major version. The release notes name the replacement.

A security fix can break a stable behaviour in a minor or patch release when there's no safe way to keep it. The release notes say what changed and why.
