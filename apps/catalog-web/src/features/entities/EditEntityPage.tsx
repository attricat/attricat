import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Button, Typography } from '@mui/material';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getCurrentBlueprint, getEntityForm, updateEntity } from './api';
import { EDIT_ENTITY_FORM_ID } from './constants';
import { draftEditors } from '../drafts/constants';
import { EditEntityToolbar } from './components/EditEntityToolbar';
import { EntityBlueprintHeaderActions } from './components/EntityBlueprintHeaderActions';
import { EntityContextPicker } from './components/EntityContextPicker';
import { EntityForm, type EntityFormHandle } from './components/EntityForm';
import { EntityAgentDrawer } from './components/EntityAgentDrawer';
import { EntityHeading } from './components/EntityHeading';
import { EntityPageEyebrow } from './components/EntityPageEyebrow';
import { EntitySchemaSubheader } from './components/EntitySchemaSubheader';
import { ReusableAttributeAttachDialog } from './components/ReusableAttributeAttachDialog';
import { SmartFillButton } from './components/SmartFillButton';
import { useReusableAttributeAttachment } from './components/useReusableAttributeAttachment';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { valuesForForm } from './entityForm';
import { checkViolationError } from '../../api/checkViolations';
import { statusParentContexts } from './status';
import { entityQueryKeys } from './queryKeys';
import { entityStatusTransitionsOptions } from './queryOptions';
import { invalidateEntity } from './invalidateEntity';
import {
  useEntityContextSelection,
  useResolvedEntityPreview,
} from './useEntityContexts';
import { lexiconText } from '../lexicon/lexicon';

export const EditEntityPage = ({ entityId }: { entityId: string }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const navigate = useNavigate({ from: '/entities/$entityId/edit' });
  const [agentOpen, setAgentOpen] = useState(false);
  const entityFormRef = useRef<EntityFormHandle>(null);
  const [reusableDialogOpen, setReusableDialogOpen] = useState(false);
  // Remounts the dialog on every opening so its selections start empty.
  const [reusableDialogSession, setReusableDialogSession] = useState(0);
  const openReusableDialog = () => {
    setReusableDialogSession((session) => session + 1);
    setReusableDialogOpen(true);
  };
  const entityForm = useQuery({
    queryKey: entityQueryKeys.form(entityId),
    queryFn: ({ signal }) => getEntityForm(entityId, signal),
    refetchOnMount: 'always',
  });
  const update = useMutation({
    mutationFn: (input: Parameters<typeof updateEntity>[1]) =>
      updateEntity(entityId, input),
    onSuccess: async (entity) => {
      entityFormRef.current?.clearDraft();
      await invalidateEntity(client, entity.id);
      void navigate({
        to: '/entities/$entityId',
        params: { entityId: entity.id },
      });
    },
  });
  const reusable = useReusableAttributeAttachment(entityId, () => {
    setReusableDialogOpen(false);
    void invalidateEntity(client, entityId);
  });
  const blueprintId = entityForm.data?.entity.blueprint_id;
  const currentBlueprint = useQuery({
    queryKey: entityQueryKeys.currentBlueprint(blueprintId ?? ''),
    queryFn: () => getCurrentBlueprint(blueprintId!),
    enabled: Boolean(blueprintId),
  });
  const { contextId, contexts, defaultContextId, setSelectedContext } =
    useEntityContextSelection();
  const resolvedPreview = useResolvedEntityPreview(entityId, contextId);
  // Explains which transitions this user may take; the server still decides.
  const statusTransitions = useQuery({
    ...entityStatusTransitionsOptions(entityId, contextId),
    enabled: contextId !== null,
  });
  const blueprint = entityForm.data?.blueprint.blueprint;
  const schemaOutdated =
    currentBlueprint.data && entityForm.data
      ? currentBlueprint.data.blueprint.version >
        (entityForm.data.entity.blueprint_version ?? Infinity)
      : undefined;
  // Check violations are shown on the form fields they involve.
  const updateChecksFailed = Boolean(checkViolationError(update.error));
  const error =
    entityForm.error ??
    (updateChecksFailed ? null : update.error) ??
    reusable.error;
  // A cached snapshot is useful for headings, but must not become the editor's
  // concurrency baseline before the opening refresh has completed.
  const editorReady = entityForm.isFetchedAfterMount;

  return (
    <PageContainer>
      <PageHeader
        actions={
          entityForm.data && (
            <EntityBlueprintHeaderActions
              blueprint={{
                id: entityForm.data.entity.blueprint_id,
                name: lexiconText(entityForm.data.blueprint.blueprint.name),
              }}
              isSample={entityForm.data.entity.is_sample}
            />
          )
        }
        eyebrow={
          <EntityPageEyebrow
            blueprint={blueprint}
            label={t('entities.editEntity')}
          />
        }
      />
      {entityForm.data && resolvedPreview.data && (
        <EntityHeading
          attributes={entityForm.data.blueprint.attributes}
          entityId={entityId}
          values={resolvedPreview.data.values}
          view={blueprint?.views?.detail}
        />
      )}
      <EditEntityToolbar
        blueprint={blueprint}
        entityId={entityId}
        saving={update.isPending}
        schemaOutdated={schemaOutdated}
      />
      <EntitySchemaSubheader
        entityId={entityId}
        name={blueprint && lexiconText(blueprint.name)}
      />
      {(entityForm.isPending || (!editorReady && entityForm.isFetching)) && (
        <Typography sx={{ mt: 4 }}>{t('entities.loadingEntity')}</Typography>
      )}
      {error && (
        <Alert severity="error" sx={{ mt: 4 }}>
          {error.message}
        </Alert>
      )}
      {resolvedPreview.isError && (
        <Alert severity="error" sx={{ mt: 4 }}>
          {resolvedPreview.error.message}
        </Alert>
      )}
      {entityForm.data && editorReady && (
        <>
          <ReusableAttributeAttachDialog
            attachAttributeDisabled={
              reusable.attach.isPending || reusable.attributesUnavailable
            }
            attachGroupDisabled={
              reusable.attachGroup.isPending || reusable.groupsUnavailable
            }
            attributes={reusable.attributes}
            groups={reusable.groups}
            key={reusableDialogSession}
            onAttachAttribute={(revisionId) =>
              reusable.attach.mutate(revisionId)
            }
            onAttachGroup={(groupId) => reusable.attachGroup.mutate(groupId)}
            onClose={() => setReusableDialogOpen(false)}
            open={reusableDialogOpen}
          />
          <EntityForm
            key={`${entityForm.data.entity.id}:${contextId ?? ''}`}
            blueprint={entityForm.data.blueprint}
            disabled={!entityForm.data.can_write}
            expectedUpdatedAt={entityForm.data.entity.updated_at}
            statusTransitions={statusTransitions.data}
            statusParentContextIds={statusParentContexts(
              contexts.data ?? [],
              contextId,
            )}
            contextId={contextId}
            draft={
              contextId
                ? {
                    editor: draftEditors.entityEdit,
                    resource: [entityId, contextId],
                    source: String(entityForm.data.entity.blueprint_version),
                  }
                : undefined
            }
            contextPicker={
              <EntityContextPicker
                contexts={contexts.data ?? []}
                disabled={contexts.isPending}
                onChange={setSelectedContext}
                value={contextId ?? ''}
              />
            }
            entityId={entityId}
            error={updateChecksFailed ? update.error : null}
            defaultContextId={defaultContextId}
            formId={EDIT_ENTITY_FORM_ID}
            footerActions={
              <Button onClick={openReusableDialog} variant="outlined">
                {t('entities.addReusableAttributeOrGroup')}
              </Button>
            }
            existingValues={[
              ...entityForm.data.values,
              ...entityForm.data.reusable_values,
            ]}
            reusableAttributes={entityForm.data.reusable_attributes}
            resolvedValues={resolvedPreview.data?.values}
            ref={entityFormRef}
            showBlueprintMetadata={false}
            showSubmitButton={false}
            initialValues={valuesForForm(
              [
                ...entityForm.data.blueprint.attributes,
                ...entityForm.data.reusable_attributes,
              ],
              [...entityForm.data.values, ...entityForm.data.reusable_values],
              contextId,
            )}
            isLoadingBlueprint={update.isPending}
            onSubmit={(input) => update.mutate(input)}
            submitLabel={t('entities.saveChanges')}
          />
        </>
      )}
      {entityForm.data && contextId && (
        <EntityAgentDrawer
          key={`${entityId}:${contextId}`}
          entityId={entityId}
          contextId={contextId}
          open={agentOpen}
          onClose={() => setAgentOpen(false)}
          draft={{
            entityId,
            contextId,
            defaultContextId,
            getDraftValues: () => entityFormRef.current?.getDraftValues() ?? {},
            onApply: (fields) =>
              entityFormRef.current?.applySmartFillValues(fields),
          }}
        />
      )}
      {entityForm.data && (
        <SmartFillButton
          disabled={update.isPending || contextId === null}
          onClick={() => setAgentOpen(true)}
        />
      )}
    </PageContainer>
  );
};
