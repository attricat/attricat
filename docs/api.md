# API Reference

The API is JSON over HTTP. Successful responses are JSON; failures use an
`error` object with a machine-readable code, message, and HTTP status. The
[CLI](cli.md) is the preferred interface for shell automation.

## Routes

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/health` | Confirm the migrated API is ready. |
| `GET` | `/blueprints` | List published entity blueprints. |
| `GET` | `/blueprints/catalogue` | List blueprint families and revisions. |
| `POST` | `/blueprints` | Create the first draft revision from TOML. |
| `GET`, `POST` | `/blueprints/{id}/versions` | List revisions or create the next draft. |
| `GET` | `/blueprints/{id}` | Read the highest published revision. |
| `GET` | `/blueprints/{id}/versions/{version}` | Read an exact revision, including drafts. |
| `POST` | `/blueprints/{id}/versions/{version}/publish` | Publish a draft revision. |
| `GET` | `/blueprints/by-code/{code}` | Read the highest published revision by code. |
| `GET` | `/blueprints/by-code/{code}/versions/{version}` | Read an exact revision by code. |
| `GET`, `POST` | `/contexts` | List or create contexts. |
| `GET` | `/contexts/{code}` | Read a context. |
| `PUT`, `DELETE` | `/contexts/id/{id}` | Update or delete a context. |
| `GET`, `POST` | `/entities` | Browse relationship targets or create an entity with the lower-level API. |
| `GET`, `DELETE` | `/entities/{id}` | Read or soft-delete an entity. |
| `GET` | `/entities/{id}/preview` | Read direct contextual preview values. |
| `GET` | `/entities/{id}/resolved-preview` | Resolve values through a requested context's ancestors. |
| `POST` | `/entities/{id}/values` | Append value history. |
| `GET` | `/entities/{id}/values/current` | Read current direct values and edges. |
| `POST` | `/entities/{id}/relationships/replace` | Replace relationship target sets. |
| `POST` | `/entities/{id}/relationships/remove` | Remove relationship targets. |
| `POST` | `/v1/entities/search` | Search entities in one blueprint revision. |
| `POST` | `/v1/entities` | Create an entity atomically with form values. |
| `GET`, `PUT` | `/v1/entities/{id}` | Read or update an entity form atomically. |
| `POST` | `/v1/entities/{id}/blueprint-migration/preview` | Assess migration to the highest published revision. |
| `POST` | `/v1/entities/{id}/blueprint-migration` | Migrate an entity to that revision. |
| `GET` | `/data-health/summary` | Read aggregate data-health metrics. |
| `GET` | `/data-health/blueprints` | Read blueprint health metrics. |
| `GET` | `/data-health/freshness` | Read value freshness metrics. |
| `GET` | `/data-health/completeness` | Read completeness metrics. |
| `GET` | `/data-health/contexts` | Read context metrics. |
| `GET` | `/data-health/relationships` | Read relationship metrics. |
| `GET` | `/data-health/storage` | Read storage metrics. |
| `POST` | `/data-health/refresh` | Clear cached data-health responses. |

Blueprint creation and revision routes create drafts. Only published revisions
can create entities or serve as migration targets. See [Blueprint Publication](database.md#blueprint-publication).
