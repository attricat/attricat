//! One page of record search results for a resolved [`SearchScope`], shared
//! by the records API and the agent's search tool: version scoping, the
//! capped totals, sort and cursor resolution, the page itself and table
//! value hydration.
use std::{collections::HashMap, time::Instant};

use tracing::Instrument;

use super::{
    AttricatRepository, RecordSearchScope, RepositoryError, SearchScope, SearchScopeError,
    decode_search_cursor, search_scope::has_extension_table_renderer, table_relationship_targets,
    table_value_paths,
};
use crate::model::{
    ExportStatusFilter, RecordSearchResponse, SearchResultVersionScope, SearchSort,
};

/// Totals and hidden-outdated counts stop at this many records.
pub const SEARCH_TOTAL_COUNT_CAP: i64 = 500;

pub struct SearchPageRequest<'a> {
    /// The blueprint code the scope was resolved for.
    pub code: &'a str,
    pub system_tags: &'a [String],
    pub outdated: bool,
    pub export_status: Option<&'a ExportStatusFilter>,
    pub sort: Option<&'a SearchSort>,
    pub cursor: Option<&'a str>,
    pub limit: i64,
    /// Count the matching records, up to [`SEARCH_TOTAL_COUNT_CAP`]. Cursor
    /// pages never count; clients keep the first page's total.
    pub include_total: bool,
    /// Hydrate the related records table columns traverse into each row's
    /// preview. Tables with an extension renderer always get them.
    pub include_previews: bool,
}

/// A phase of a search page that has just ended, for request timings.
pub enum SearchPhase {
    /// Candidate records, version scope and sort are resolved.
    Candidates,
    /// The page queries ran from the given instant.
    Page(Instant),
    /// Table values were hydrated from the given instant.
    Related(Instant),
}

impl AttricatRepository {
    /// One page of `scope`'s records. The response always includes the
    /// blueprint revision the rows were shaped by.
    pub async fn search_page(
        &self,
        scope: &SearchScope,
        request: SearchPageRequest<'_>,
        on_phase: &(dyn Fn(SearchPhase) + Sync),
    ) -> Result<RecordSearchResponse, SearchScopeError> {
        let SearchScope {
            current,
            selected,
            search_blueprint,
            context,
            explanations,
            matching,
        } = scope;
        let selected = *selected;
        let records = RecordSearchScope {
            blueprint_id: current.blueprint.id,
            blueprint_version: selected,
            matching_record_ids: matching.as_deref(),
            system_tags: request.system_tags,
            outdated: request.outdated,
            export_status: request.export_status,
            current_blueprint_version: current.blueprint.version,
        };
        let include_total = request.include_total && request.cursor.is_none();
        let (matching_versions, total_count) = self
            .search_result_versions_and_count(
                records,
                include_total.then_some(SEARCH_TOTAL_COUNT_CAP + 1),
            )
            .instrument(tracing::info_span!(
                "sql.operation",
                label = "result-version-scope"
            ))
            .await?;
        let effective_source_version = match matching_versions.as_slice() {
            [version] => Some(*version),
            _ => selected,
        };
        let sort_uses_relationship = request.sort.is_some_and(|sort| sort.field.contains('.'));
        if selected.is_none() && sort_uses_relationship && matching_versions.len() != 1 {
            return Err(SearchScopeError::RelationshipSortRequiresSingleVersion);
        }
        let result_blueprint = match effective_source_version {
            Some(version)
                if version != current.blueprint.version
                    && version != search_blueprint.blueprint.version =>
            {
                self.get_published_blueprint_by_code_and_version(request.code, version)
                    .await?
                    .ok_or(RepositoryError::NotFound("blueprint"))?
            }
            _ => search_blueprint.clone(),
        };
        let sort = self
            .resolve_search_sort(
                &result_blueprint,
                request.sort,
                sort_uses_relationship
                    .then_some(effective_source_version)
                    .flatten(),
                context,
            )
            .instrument(tracing::info_span!(
                "sql.operation",
                label = "table-sort-resolve"
            ))
            .await?;
        let cursor = match (sort.is_some(), request.cursor) {
            (true, _) | (false, None) => None,
            (false, Some(cursor)) => {
                Some(
                    decode_search_cursor(cursor).ok_or_else(|| SearchScopeError::Invalid {
                        message: "page.cursor is invalid".to_owned(),
                        blueprint: None,
                    })?,
                )
            }
        };
        on_phase(SearchPhase::Candidates);
        let page_started = Instant::now();
        let page = async {
            match sort.as_ref() {
                Some(sort) => {
                    let page_scope = RecordSearchScope {
                        blueprint_version: if sort_uses_relationship {
                            effective_source_version
                        } else {
                            selected
                        },
                        ..records
                    };
                    self.search_record_previews_sorted(
                        page_scope,
                        request.limit,
                        request.cursor,
                        sort,
                    )
                    .instrument(tracing::info_span!("sql.operation", label = "records-page"))
                    .await
                }
                None => {
                    self.search_record_previews(records, request.limit, cursor)
                        .instrument(tracing::info_span!("sql.operation", label = "records-page"))
                        .await
                }
            }
        };
        let hidden_outdated = async {
            if selected != Some(current.blueprint.version) || request.cursor.is_some() {
                return Ok(None);
            }
            self.count_record_previews(
                RecordSearchScope {
                    blueprint_version: None,
                    matching_record_ids: None,
                    system_tags: &[],
                    outdated: true,
                    export_status: None,
                    ..records
                },
                SEARCH_TOTAL_COUNT_CAP + 1,
            )
            .await
            .map(Some)
        };
        let ((mut items, next_cursor), hidden_outdated_count) =
            tokio::try_join!(page, hidden_outdated)?;
        on_phase(SearchPhase::Page(page_started));
        for item in &mut items {
            item.schema_outdated = item.blueprint_version != current.blueprint.version;
            item.match_explanations = explanations.get(&item.id).cloned().unwrap_or_default();
        }
        let related_started = Instant::now();
        let previews = request.include_previews || has_extension_table_renderer(&result_blueprint);
        let relationships = if previews {
            table_relationship_targets(&result_blueprint)
        } else {
            HashMap::new()
        };
        self.hydrate_table_values(
            &mut items,
            &table_value_paths(&result_blueprint),
            &relationships,
            context,
        )
        .instrument(tracing::info_span!(
            "sql.operation",
            label = "table-values-hydrate"
        ))
        .await?;
        on_phase(SearchPhase::Related(related_started));
        if !previews {
            for item in &mut items {
                item.preview = serde_json::Value::Null;
            }
        }
        let capped = |count: Option<i64>| {
            (
                count.map(|count| count.min(SEARCH_TOTAL_COUNT_CAP)),
                count.is_some_and(|count| count > SEARCH_TOTAL_COUNT_CAP),
            )
        };
        let (total_count, total_count_capped) = capped(total_count);
        let (hidden_outdated_count, hidden_outdated_count_capped) = capped(hidden_outdated_count);
        Ok(RecordSearchResponse {
            blueprint: Some(result_blueprint),
            items,
            next_cursor,
            total_count,
            total_count_capped,
            result_version_scope: match matching_versions.as_slice() {
                [] => SearchResultVersionScope::Empty,
                [version] => SearchResultVersionScope::Single { version: *version },
                _ => SearchResultVersionScope::Multiple,
            },
            hidden_outdated_count,
            hidden_outdated_count_capped,
        })
    }
}
