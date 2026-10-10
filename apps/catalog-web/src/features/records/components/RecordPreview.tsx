import { useQuery } from '@tanstack/react-query';
import { Alert, Box, CircularProgress, Typography } from '@mui/material';
import { lazy, Suspense, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { QueryErrorNotice } from '../../../components/QueryErrorNotice';
import { ApiErrorAlert } from '../../../components/CheckViolationsAlert';
import { ExtensionOutlet } from '../../extensions/ExtensionOutlet';
import { currentSession } from '../../auth/api';
import { authQueryKeys } from '../../auth/queryKeys';
import { lexiconText } from '../../lexicon/lexicon';
import { getCurrentBlueprint, type CatalogRecord } from '../api';
import {
  RECORD_HEADER_CONTEXT_VERSION,
  FALLBACK_BLUEPRINT_VERSION,
} from '../constants';
import { recordQueryKeys } from '../queryKeys';
import { statusParentContexts } from '../status';
import { useRecordContextSelection } from '../useRecordContexts';
import { useRecordPreviewData } from '../useRecordPreviewData';
import { DataQualityFindingsChip } from './DataQualityFindingsChip';
import { DeleteRecordDialog } from './DeleteRecordDialog';
import { DuplicateRecordDialog } from './DuplicateRecordDialog';
import { RecordAgentDrawer } from './RecordAgentDrawer';
import { RecordBlueprintHeaderActions } from './RecordBlueprintHeaderActions';
import { RecordExtensionDrawer } from './RecordExtensionDrawer';
import { RecordHeading } from './RecordHeading';
import type { RecordInlineFieldsHandle } from './RecordInlineFields';
import { RecordPreviewContent } from './RecordPreviewContent';
import { RecordPreviewToolbar } from './RecordPreviewToolbar';
import { RecordControlsPanel } from './RecordControlsPanel';
import { RelationshipPickerActionBar } from './RelationshipPickerActionBar';
import { useRecordPublications } from './useRecordPublications';

const RecordCommentsPanel = lazy(() =>
  import('../../record-comments/RecordCommentsPanel').then((module) => ({
    default: module.RecordCommentsPanel,
  })),
);

export type RecordPreviewFrame = {
  /** The record's blueprint revision, once loaded. */
  blueprint?: { code: string; id: string; name: string; version: number };
  /** Blueprint, sample, schema and data quality indicators. */
  headerActions: ReactNode;
  children: ReactNode;
};

type Props = {
  recordId: string;
  /** Renders smaller headings and one column of fields for a panel beside another page. */
  compact?: boolean;
  /** The context shown until another is chosen. */
  initialContextId?: string;
  /** Right edge of the agent and extension drawers, beside a panel. */
  drawerOffset?: string;
  relationshipPickerToken?: string;
  /** Comments are shown only on the record page for now. */
  showComments?: boolean;
  onDeleted: () => void;
  onDuplicated: (copy: CatalogRecord) => void;
  /** Places the preview in its page or panel. */
  renderFrame: (frame: RecordPreviewFrame) => ReactNode;
};

/**
 * A record's values, actions and side panels. The record page and the
 * Explorer's record panel differ only in their frame.
 */
export const RecordPreview = ({
  recordId,
  compact = false,
  initialContextId,
  drawerOffset,
  relationshipPickerToken,
  showComments = false,
  onDeleted,
  onDuplicated,
  renderFrame,
}: Props) => {
  const { t } = useTranslation();
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [duplicateOpen, setDuplicateOpen] = useState(false);
  const [extensionPanelOpen, setExtensionPanelOpen] = useState(false);
  const [agentPanelOpen, setAgentPanelOpen] = useState(false);
  const inlineFieldsRef = useRef<RecordInlineFieldsHandle>(null);
  const { contextId, contexts, defaultContextId, setSelectedContext } =
    useRecordContextSelection(initialContextId);
  const selectedContextId = contextId ?? undefined;
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const canPublish = session.data?.capabilities?.records_publish === true;
  const publications = useRecordPublications(recordId, contextId, canPublish);
  const { blueprint, recordForm, resolved, statusTransitions } =
    useRecordPreviewData(recordId, contextId);
  const resolvedRecord = resolved.data?.record;
  const currentBlueprint = useQuery({
    queryKey: recordQueryKeys.currentBlueprint(
      resolvedRecord?.blueprint_id ?? '',
    ),
    queryFn: () => {
      if (!resolvedRecord)
        throw new Error(t('records.recordPreviewUnavailable'));
      return getCurrentBlueprint(resolvedRecord.blueprint_id);
    },
    enabled: Boolean(resolvedRecord?.blueprint_id),
  });
  const schemaOutdated =
    currentBlueprint.data && resolvedRecord
      ? currentBlueprint.data.blueprint.version >
        resolvedRecord.blueprint_version
      : undefined;
  const actionError = publications.error;
  const loaded = resolved.data && blueprint.data;

  return (
    <>
      {renderFrame({
        blueprint: blueprint.data?.blueprint,
        headerActions: (
          <RecordBlueprintHeaderActions
            blueprint={blueprint.data?.blueprint}
            hideBlueprintLink={compact}
            schemaOutdated={schemaOutdated}
            isSample={resolvedRecord?.is_sample}
          >
            <DataQualityFindingsChip recordId={recordId} />
          </RecordBlueprintHeaderActions>
        ),
        children: (
          <>
            {relationshipPickerToken && (
              <RelationshipPickerActionBar
                recordId={recordId}
                pickerToken={relationshipPickerToken}
              />
            )}
            {resolved.data && blueprint.data && (
              <Box>
                {/* Names the kind of record; a full page names it in its
                    header's eyebrow instead. */}
                {compact && (
                  <Typography color="primary" component="p" variant="overline">
                    {lexiconText(blueprint.data.blueprint.name)}
                  </Typography>
                )}
                <RecordHeading
                  attributes={blueprint.data.attributes}
                  compact={compact}
                  recordId={recordId}
                  values={resolved.data.values}
                  view={blueprint.data.blueprint.views.detail}
                />
              </Box>
            )}
            {actionError && (
              <ApiErrorAlert error={actionError} sx={{ mt: 3 }} />
            )}
            {blueprint.data && resolved.data && (
              <ExtensionOutlet
                context={{
                  context_version: RECORD_HEADER_CONTEXT_VERSION,
                  record_id: recordId,
                  blueprint_id: blueprint.data.blueprint.id,
                  blueprint_version: blueprint.data.blueprint.version,
                }}
                outlet="record_header_action"
                runtimeScope={{
                  blueprintId: blueprint.data.blueprint.id,
                  blueprintVersion: blueprint.data.blueprint.version,
                }}
              />
            )}
            <RecordPreviewToolbar
              compact={compact}
              recordId={recordId}
              extensionPanelOpen={extensionPanelOpen}
              onOpenExtensions={() => {
                setAgentPanelOpen(false);
                setExtensionPanelOpen(true);
              }}
              agentPanelOpen={agentPanelOpen}
              onOpenAgent={() => {
                setExtensionPanelOpen(false);
                setAgentPanelOpen(true);
              }}
              schemaOutdated={schemaOutdated}
              showExtensions={Boolean(loaded)}
              onDuplicate={() => setDuplicateOpen(true)}
              canDelete={
                session.data?.capabilities?.records_delete === true &&
                Boolean(resolved.data)
              }
              onDelete={() => setDeleteOpen(true)}
              publication={publications.publication}
              canPublish={canPublish}
              readiness={publications.readiness}
              notReadyChannels={publications.notReadyChannels}
              onPublish={publications.publish}
              onPublishAll={publications.publishAll}
              onUnpublish={publications.unpublish}
              publicationPending={publications.isPending}
            />
            {contexts.isPending && (
              <Box sx={{ display: 'flex', justifyContent: 'center', py: 3 }}>
                <CircularProgress
                  aria-label={t('records.loadingContexts')}
                  enableTrackSlot
                />
              </Box>
            )}
            <QueryErrorNotice
              error={contexts.error}
              isRetrying={contexts.isFetching}
              onRetry={() => void contexts.refetch()}
            />
            {contexts.data && (
              <>
                {resolved.isPending && (
                  <Box
                    sx={{ display: 'flex', justifyContent: 'center', mt: 3 }}
                  >
                    <CircularProgress
                      aria-label={t('records.resolvingValues')}
                      enableTrackSlot
                    />
                  </Box>
                )}
                {resolved.isError && (
                  <Alert severity="error" sx={{ mt: 3 }}>
                    {resolved.error.message}
                  </Alert>
                )}
                {blueprint.isError && (
                  <Alert severity="error" sx={{ mt: 3 }}>
                    {blueprint.error.message}
                  </Alert>
                )}
                {resolved.data && blueprint.data && (
                  <RecordPreviewContent
                    blueprint={blueprint.data}
                    contextId={selectedContextId}
                    contexts={contexts.data}
                    defaultContextId={defaultContextId}
                    recordId={recordId}
                    // A cached form must not become the save baseline before
                    // the opening refresh has completed.
                    form={
                      recordForm.isFetchedAfterMount
                        ? recordForm.data
                        : undefined
                    }
                    statusParentContextIds={statusParentContexts(
                      contexts.data,
                      contextId,
                    )}
                    statusTransitions={statusTransitions.data}
                    inlineFieldsRef={inlineFieldsRef}
                    onContextChange={setSelectedContext}
                    resolved={resolved.data}
                    singleColumn={compact}
                  />
                )}
              </>
            )}
            {resolved.data && (
              <RecordControlsPanel
                attributes={blueprint.data?.attributes ?? []}
                recordId={recordId}
              />
            )}
            {showComments && resolved.data && (
              <Suspense
                fallback={
                  <CircularProgress aria-label={t('comments.loading')} />
                }
              >
                <RecordCommentsPanel key={recordId} recordId={recordId} />
              </Suspense>
            )}
          </>
        ),
      })}
      {duplicateOpen && (
        <DuplicateRecordDialog
          recordId={recordId}
          onClose={() => setDuplicateOpen(false)}
          onDuplicated={(copy) => {
            setDuplicateOpen(false);
            onDuplicated(copy);
          }}
        />
      )}
      {deleteOpen && (
        <DeleteRecordDialog
          recordId={recordId}
          onClose={() => setDeleteOpen(false)}
          onDeleted={() => {
            setDeleteOpen(false);
            onDeleted();
          }}
        />
      )}
      <RecordAgentDrawer
        key={`${recordId}:${selectedContextId ?? ''}`}
        recordId={recordId}
        contextId={selectedContextId}
        offset={drawerOffset}
        onClose={() => setAgentPanelOpen(false)}
        open={agentPanelOpen}
        // Suggestions apply to the fields only when this user can edit them.
        draft={
          recordForm.data?.can_write && contextId
            ? {
                recordId,
                contextId,
                defaultContextId,
                getDraftValues: () =>
                  inlineFieldsRef.current?.getDraftValues() ?? {},
                onApply: (fields) =>
                  inlineFieldsRef.current?.applySmartFillValues(fields),
              }
            : undefined
        }
      />
      <RecordExtensionDrawer
        blueprintId={resolvedRecord?.blueprint_id ?? ''}
        blueprintVersion={
          resolvedRecord?.blueprint_version ?? FALLBACK_BLUEPRINT_VERSION
        }
        contextId={selectedContextId}
        recordId={recordId}
        offset={drawerOffset}
        onClose={() => setExtensionPanelOpen(false)}
        open={extensionPanelOpen}
        showContent={Boolean(loaded)}
      />
    </>
  );
};
