//! Connector job calls of attricat:host 1.1: `attricat.page-v2`,
//! `attricat.withdrawn` and annotation writes inside scoped connector runs.
//! Each reads the run's host-selected blueprint, context and channel; the
//! component never names them.

use super::*;
use crate::repository::{ConnectorPageV2Request, ExtensionAttricatBatch};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageV2Request {
    #[serde(default)]
    cursor: String,
    limit: u32,
    /// Relationship attribute codes whose targets' values to embed.
    #[serde(default)]
    expand: Vec<String>,
}

// Borrow only the Sync repository: `OperationState` holds a resource table
// and is not Sync, so host futures cannot capture `&OperationState`.
async fn export_scope(
    repository: &AttricatRepository,
    run_id: Uuid,
) -> Result<crate::repository::ConnectorRunDetails, String> {
    match repository
        .connector_run_details(run_id)
        .await
        .map_err(|error| error.to_string())?
    {
        Some(details) if details.direction == "export" => Ok(details),
        Some(_) => Err("this call needs an export connector job run".into()),
        None => Err("this call is available only in connector job runs".into()),
    }
}

impl OperationState {
    pub(super) async fn attricat_page_v2(&mut self, request: String) -> Result<String, String> {
        let input: PageV2Request = parse_host_request(&request, "page request")?;
        let mut host = HostState::new(self.installation.clone(), self.repository.clone(), 0);
        host.require_active("attricat.read").await?;
        if input.limit == 0 || input.limit > MAX_CONNECTOR_PAGE_LIMIT {
            return Err(format!(
                "connector page limit must be 1-{MAX_CONNECTOR_PAGE_LIMIT}"
            ));
        }
        let scope = export_scope(&self.repository, self.run_id).await?;
        let page = self
            .repository
            .extension_connector_page_v2(ConnectorPageV2Request {
                blueprint_id: scope.blueprint_id,
                blueprint_version: scope.blueprint_version,
                context_id: scope.context_id,
                channel_id: scope.channel_id,
                cursor: (!input.cursor.is_empty()).then_some(input.cursor),
                limit: input.limit,
                expand: input.expand,
                record_ids: scope.record_ids,
            })
            .await
            .map_err(|error| error.to_string())?;
        bounded_serialize(&page).map_err(|_| {
            "the page exceeds the host JSON limit; request fewer rows or expand fewer relationships"
                .to_owned()
        })
    }

    /// `connector.scope.v1`: the run's job, blueprint, context, channel,
    /// connection and mode. `null` outside connector job runs.
    pub(super) async fn connector_scope(&mut self) -> Result<String, String> {
        let details = self
            .repository
            .connector_run_details(self.run_id)
            .await
            .map_err(|error| error.to_string())?;
        bounded_serialize(&details)
    }

    pub(super) async fn attricat_withdrawn(
        &mut self,
        cursor: String,
        limit: u32,
    ) -> Result<String, String> {
        let mut host = HostState::new(self.installation.clone(), self.repository.clone(), 0);
        host.require_active("attricat.read").await?;
        let scope = export_scope(&self.repository, self.run_id).await?;
        let page = self
            .repository
            .extension_withdrawn_records(
                scope.blueprint_id,
                scope.channel_id,
                scope.record_ids.as_deref(),
                Some(cursor.as_str()),
                limit,
            )
            .await
            .map_err(|error| error.to_string())?;
        bounded_serialize(&page)
    }

    /// Applies an annotation-only batch inside a scoped connector run: an
    /// exporter records external IDs and sync status on records of its
    /// job's blueprint. Value writes stay unavailable to export runs.
    pub(super) async fn scoped_annotation_batch(
        &mut self,
        blueprint_id: Uuid,
        batch: ExtensionAttricatBatch,
    ) -> Result<String, String> {
        if batch.intents.is_empty()
            || !batch
                .intents
                .iter()
                .all(ExtensionAttricatIntent::is_annotation)
        {
            return Err("scoped jobs must use host-filtered connector catalog calls".into());
        }
        let mut record_ids = batch
            .intents
            .iter()
            .map(|intent| {
                intent
                    .target_record_id()
                    .ok_or_else(|| "annotation intents need a record".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        record_ids.sort_unstable();
        record_ids.dedup();
        let in_scope = self
            .repository
            .records_of_blueprint(blueprint_id, &record_ids)
            .await
            .map_err(|error| error.to_string())?;
        if in_scope.len() != record_ids.len() {
            return Err("annotation targets a record outside the job's blueprint".into());
        }
        // A run confined to selected records annotates only those.
        if let Some(selection) = self
            .repository
            .connector_run_details(self.run_id)
            .await
            .map_err(|error| error.to_string())?
            .and_then(|details| details.record_ids)
            && record_ids.iter().any(|id| !selection.contains(id))
        {
            return Err("annotation targets a record outside the run's selection".into());
        }
        let mut host = HostState::new(self.installation.clone(), self.repository.clone(), 0);
        host.require_all_active(batch_capabilities(&batch)).await?;
        let repository = host
            .repository
            .for_extension(&host.installation.extension_id);
        let outcomes = AttricatMutationService::new(&repository)
            .execute_extension_attricat_batch(batch)
            .await
            .map_err(|error| error.to_string())?;
        bounded_serialize(&outcomes)
    }
}
