import { useQuery } from '@tanstack/react-query';
import { Box, CircularProgress, Typography } from '@mui/material';
import {
  lazy,
  Suspense,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { useTranslation } from 'react-i18next';
import { QueryErrorNotice } from '../../../components/QueryErrorNotice';
import { ApiErrorAlert } from '../../../components/ApiErrorAlert';
import { BlueprintLink } from '../../blueprints/BlueprintLink';
import { ExtensionOutlet } from '../../extensions/ExtensionOutlet';
import { useCan, useCanOn } from '../../auth/useCan';
import { type AttricatRecord, type Attribute } from '../api';
import {
  attributeCardinalities,
  FALLBACK_BLUEPRINT_VERSION,
  recordPreviewTabs,
  type RecordPreviewTabKey,
} from '../constants';
import { attributeValueTypes } from '../valueTypes';
import { commentsSectionId } from '../../record-comments/constants';
import { RecordCommentsLink } from '../../record-comments/RecordCommentsLink';
import { RelatedRecordPreviewContext } from '../linkedRecord';
import { pageTitleLabel } from '../pageTitle';
import { useRecordReadAhead } from '../useRecordReadAhead';
import {
  KnownRecordLabelsContext,
  relationshipTargetLabels,
} from '../knownRecordLabels';
import { statusParentContexts } from '../status';
import { useRecordContextSelection } from '../useRecordContexts';
import { useRecordPreviewData } from '../useRecordPreviewData';
import { RecordFindingsPanel } from '../../rules/RecordFindingsPanel';
import { DataQualityFindingsChip } from './DataQualityFindingsChip';
import { RecordExportStatusChip } from '../../extension-exports/RecordExportStatusChip';
import { RecordExportStatusPanel } from '../../extension-exports/RecordExportStatusPanel';
import { DeleteRecordDialog } from './DeleteRecordDialog';
import { DuplicateRecordDialog } from './DuplicateRecordDialog';
import { RecordAgentDrawer } from './RecordAgentDrawer';
import { RecordConversationsPanel } from '../../agents/RecordConversationsPanel';
import { RecordBlueprintHeaderActions } from './RecordBlueprintHeaderActions';
import { RecordExtensionDrawer } from './RecordExtensionDrawer';
import { RecordHeading } from './RecordHeading';
import type { RecordInlineFieldsHandle } from './RecordInlineFields';
import { RecordPreviewContent } from './RecordPreviewContent';
import {
  FieldRevealContext,
  RevealFieldContext,
  useFieldReveal,
} from '../../views/fieldReveal';
import { RecordPreviewTabs } from './RecordPreviewTabs';
import { RecordPreviewToolbar } from './RecordPreviewToolbar';
import { RecordControlsPanel } from './RecordControlsPanel';
import { RecordSystemAnnotationsPanel } from './RecordSystemAnnotationsPanel';
import { visibleSystemTags } from '../systemTags';
import { SystemTagChips } from './SystemTagChips';
import { RelationshipPickerActionBar } from './RelationshipPickerActionBar';
import { useRecordPublications } from './useRecordPublications';
import { capabilities, accessTargetKinds } from '../../auth/capabilities';
import { currentBlueprintOptions } from '../queryOptions';
import {
  extensionOutlets,
  supportedOutletContextVersion,
} from '../../extensions/constants';

const loadRecordCommentsPanel = () =>
  import('../../record-comments/RecordCommentsPanel');
const RecordCommentsPanel = lazy(() =>
  loadRecordCommentsPanel().then((module) => ({
    default: module.RecordCommentsPanel,
  })),
);

export type RecordPreviewFrame = {
  /** The record's blueprint revision, once loaded. */
  blueprint?: { code: string; id: string; name: string; version: number };
  /**
   * The record's `page_title` view label, `null` when its blueprint has none
   * or the fields are empty, or `undefined` until the record has loaded.
   */
  title?: string | null;
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
  /** Shows the record's comments in a tab. */
  showComments?: boolean;
  onDeleted: () => void;
  onDuplicated: (copy: AttricatRecord) => void;
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
  // The conversation in the agent panel, or `null` to start a new one.
  const [agentConversationId, setAgentConversationId] = useState<string | null>(
    null,
  );
  const inlineFieldsRef = useRef<RecordInlineFieldsHandle>(null);
  // Leads to a field from its save status, a connected field's diagnostics
  // or a data quality finding about it.
  const fieldsRef = useRef<HTMLDivElement>(null);
  const { request: revealRequest, reveal: revealField } =
    useFieldReveal(fieldsRef);
  // A link to the comments opens their tab.
  const [tab, setTab] = useState<RecordPreviewTabKey>(() =>
    showComments && window.location.hash === `#${commentsSectionId}`
      ? recordPreviewTabs.comments
      : recordPreviewTabs.dataQuality,
  );
  const tabPanelRef = useRef<HTMLDivElement>(null);
  // Counts requests to bring the selected tab into view once it renders.
  const [tabReveal, setTabReveal] = useState(0);
  useEffect(() => {
    if (tabReveal === 0) return;
    const panel = tabPanelRef.current;
    panel?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    panel?.focus({ preventScroll: true });
  }, [tabReveal]);
  const { contextId, contexts, defaultContextId, setSelectedContext } =
    useRecordContextSelection(initialContextId);
  const selectedContextId = contextId ?? undefined;
  const canPublish = useCanOn(
    capabilities.recordsPublish,
    accessTargetKinds.record,
    recordId,
  );
  const canDeleteRecords = useCanOn(
    capabilities.recordsDelete,
    accessTargetKinds.record,
    recordId,
  );
  const canCreateRecords = useCan(capabilities.recordsWrite);
  const canAskAgent = useCan(capabilities.agentsRun);
  const publications = useRecordPublications(recordId, contextId, canPublish);
  const { blueprint, recordForm, resolved, statusTransitions } =
    useRecordPreviewData(recordId, contextId);
  useRecordReadAhead(
    recordId,
    showComments,
    // A panel beside another page keeps related records to links.
    // The form arrives first and carries the same revision.
    compact
      ? []
      : previewedRelationshipBlueprints(
          blueprint.data?.attributes ?? recordForm.data?.blueprint.attributes,
        ),
  );
  // The comments chunk downloads while the record loads.
  useEffect(() => {
    if (showComments) void loadRecordCommentsPanel();
  }, [showComments]);
  // A finding about other contexts opens the first of them, then its field
  // once the record's values there have loaded.
  const [pendingReveal, setPendingReveal] = useState<{
    code: string;
    contextId: string;
  }>();
  const revealFindingField = (
    code: string,
    contextCodes: readonly string[],
  ) => {
    const shown = contexts.data?.find((context) => context.id === contextId);
    const target = contexts.data?.find((context) =>
      contextCodes.includes(context.code),
    );
    if (!target || !shown || contextCodes.includes(shown.code)) {
      revealField(code);
      return;
    }
    setSelectedContext(target.id);
    setPendingReveal({ code, contextId: target.id });
  };
  // The reveal is dropped when another context is chosen first or its values
  // cannot be loaded.
  if (
    pendingReveal !== undefined &&
    (pendingReveal.contextId !== contextId ||
      (resolved.isError && !resolved.isFetching))
  )
    setPendingReveal(undefined);
  else if (
    pendingReveal !== undefined &&
    resolved.isSuccess &&
    !resolved.isFetching
  ) {
    setPendingReveal(undefined);
    revealField(pendingReveal.code);
  }
  const resolvedRecord = resolved.data?.record;
  const systemTags = recordForm.data?.record.system_tags ?? [];
  const knownLabels = useMemo(
    () =>
      resolved.data
        ? relationshipTargetLabels(
            resolved.data,
            contextId !== null && contextId === defaultContextId,
          )
        : new Map<string, string>(),
    [contextId, defaultContextId, resolved.data],
  );
  const currentBlueprint = useQuery(
    currentBlueprintOptions(resolvedRecord?.blueprint_id),
  );
  const schemaOutdated =
    currentBlueprint.data && resolvedRecord
      ? currentBlueprint.data.blueprint.version >
        resolvedRecord.blueprint_version
      : undefined;
  const actionError = publications.error;
  const contextCodes = useMemo(
    () =>
      new Map(
        (contexts.data ?? []).map((context) => [context.id, context.code]),
      ),
    [contexts.data],
  );
  const openAgent = (conversationId: string | null) => {
    setAgentConversationId(conversationId);
    setExtensionPanelOpen(false);
    setAgentPanelOpen(true);
  };
  const showTab = (key: RecordPreviewTabKey) => {
    setTab(key);
    setTabReveal((count) => count + 1);
  };
  const loaded = resolved.data && blueprint.data;

  return (
    <>
      {renderFrame({
        blueprint: blueprint.data?.blueprint,
        title:
          resolved.data && blueprint.data
            ? pageTitleLabel(
                blueprint.data.blueprint.views,
                blueprint.data.attributes,
                resolved.data.values,
              )
            : undefined,
        headerActions: (
          <RecordBlueprintHeaderActions
            blueprint={blueprint.data?.blueprint}
            hideBlueprintLink={compact}
            schemaOutdated={schemaOutdated}
            isSample={resolvedRecord?.is_sample}
          >
            <DataQualityFindingsChip
              recordId={recordId}
              onOpen={() => showTab(recordPreviewTabs.dataQuality)}
            />
            <RecordExportStatusChip
              recordId={recordId}
              onOpen={() => showTab(recordPreviewTabs.exports)}
            />
            {/* A panel's tabs sit below the fold; the count leads to them. */}
            {compact && showComments && (
              <RecordCommentsLink
                recordId={recordId}
                onOpen={() => showTab(recordPreviewTabs.comments)}
              />
            )}
          </RecordBlueprintHeaderActions>
        ),
        children: (
          <RevealFieldContext value={revealField}>
            <FieldRevealContext value={revealRequest}>
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
                    <Typography
                      color="primary"
                      component="p"
                      variant="overline"
                    >
                      <BlueprintLink
                        blueprint={blueprint.data.blueprint}
                        style={{ textDecoration: 'none' }}
                      />
                    </Typography>
                  )}
                  <RecordHeading
                    attributes={blueprint.data.attributes}
                    compact={compact}
                    contextId={selectedContextId}
                    recordId={recordId}
                    values={resolved.data.values}
                    view={blueprint.data.blueprint.views.detail}
                  />
                  {visibleSystemTags(systemTags).length > 0 && (
                    <Box
                      aria-label={t('records.systemAnnotations.tags')}
                      role="group"
                      sx={{ mt: 1 }}
                    >
                      <SystemTagChips tags={systemTags} />
                    </Box>
                  )}
                </Box>
              )}
              <ApiErrorAlert error={actionError} sx={{ mt: 3 }} />
              {blueprint.data && resolved.data && (
                <ExtensionOutlet
                  context={{
                    context_version: supportedOutletContextVersion,
                    record_id: recordId,
                    blueprint_id: blueprint.data.blueprint.id,
                    blueprint_version: blueprint.data.blueprint.version,
                  }}
                  outlet={extensionOutlets.recordHeaderAction}
                  runtimeScope={{
                    blueprintId: blueprint.data.blueprint.id,
                    blueprintVersion: blueprint.data.blueprint.version,
                  }}
                />
              )}
              <RecordPreviewToolbar
                blueprint={blueprint.data?.blueprint}
                compact={compact}
                recordId={recordId}
                extensionPanelOpen={extensionPanelOpen}
                onOpenExtensions={() => {
                  setAgentPanelOpen(false);
                  setExtensionPanelOpen(true);
                }}
                agentPanelOpen={agentPanelOpen}
                onOpenAgent={() => openAgent(null)}
                schemaOutdated={schemaOutdated}
                showExtensions={Boolean(loaded)}
                canWrite={recordForm.data?.can_write === true}
                canCreate={canCreateRecords}
                canAskAgent={canAskAgent}
                onDuplicate={() => setDuplicateOpen(true)}
                canDelete={canDeleteRecords && Boolean(resolved.data)}
                onDelete={() => setDeleteOpen(true)}
                canPublish={canPublish}
                publications={publications}
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
                  <ApiErrorAlert error={resolved.error} sx={{ mt: 3 }} />
                  <ApiErrorAlert error={blueprint.error} sx={{ mt: 3 }} />
                  {resolved.data && blueprint.data && (
                    // A single relationship shows its record on the full page;
                    // a panel beside another page keeps to links.
                    <Box ref={fieldsRef}>
                      <KnownRecordLabelsContext value={knownLabels}>
                        <RelatedRecordPreviewContext.Provider
                          value={compact ? null : (contextId ?? null)}
                        >
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
                        </RelatedRecordPreviewContext.Provider>
                      </KnownRecordLabelsContext>
                    </Box>
                  )}
                </>
              )}
              {resolvedRecord && (
                <RecordPreviewTabs
                  onChange={setTab}
                  panelRef={tabPanelRef}
                  tabs={[
                    {
                      key: recordPreviewTabs.dataQuality,
                      label: t('rules.recordFindings.title'),
                      content: (
                        <RecordFindingsPanel
                          attributes={blueprint.data?.attributes}
                          blueprintId={resolvedRecord.blueprint_id}
                          blueprintVersion={resolvedRecord.blueprint_version}
                          contextCodes={contexts.data?.map(
                            (context) => context.code,
                          )}
                          onRevealField={revealFindingField}
                          recordId={recordId}
                        />
                      ),
                    },
                    ...(showComments
                      ? [
                          {
                            key: recordPreviewTabs.comments,
                            label: t('comments.title'),
                            content: (
                              <Suspense
                                fallback={
                                  <CircularProgress
                                    aria-label={t('comments.loading')}
                                  />
                                }
                              >
                                <RecordCommentsPanel
                                  key={recordId}
                                  recordId={recordId}
                                />
                              </Suspense>
                            ),
                          },
                        ]
                      : []),
                    ...(canAskAgent
                      ? [
                          {
                            key: recordPreviewTabs.conversations,
                            label: t('records.agentConversations.title'),
                            content: (
                              <RecordConversationsPanel
                                key={recordId}
                                contextCodes={contextCodes}
                                onOpen={openAgent}
                                onStart={() => openAgent(null)}
                                recordId={recordId}
                              />
                            ),
                          },
                        ]
                      : []),
                    {
                      key: recordPreviewTabs.recordControl,
                      label: t('records.recordControls.title'),
                      content: (
                        <RecordControlsPanel
                          attributes={blueprint.data?.attributes ?? []}
                          recordId={recordId}
                        />
                      ),
                    },
                    {
                      key: recordPreviewTabs.systemAnnotations,
                      label: t('records.systemAnnotations.title'),
                      content: (
                        <RecordSystemAnnotationsPanel
                          metadata={
                            recordForm.data?.record.system_metadata ?? {}
                          }
                          tags={systemTags}
                        />
                      ),
                    },
                    {
                      key: recordPreviewTabs.exports,
                      label: t('extensionExports.recordTab'),
                      content: <RecordExportStatusPanel recordId={recordId} />,
                    },
                  ]}
                  value={tab}
                />
              )}
            </FieldRevealContext>
          </RevealFieldContext>
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
        conversationId={agentConversationId}
        onConversationChange={setAgentConversationId}
        onShowAll={
          resolvedRecord
            ? () => showTab(recordPreviewTabs.conversations)
            : undefined
        }
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

/** Target blueprints of the relationships whose record a full page previews. */
const previewedRelationshipBlueprints = (attributes: Attribute[] = []) =>
  attributes.flatMap((attribute) =>
    attribute.value_type === attributeValueTypes.relationship &&
    attribute.cardinality === attributeCardinalities.one &&
    attribute.target_blueprint_code
      ? [attribute.target_blueprint_code]
      : [],
  );
