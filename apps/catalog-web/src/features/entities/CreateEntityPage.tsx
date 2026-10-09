import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useRef } from 'react';
import { useNavigate } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { listContexts } from '../contexts/api';
import { contextQueryKeys } from '../contexts/queryKeys';
import { defaultContextCode } from '../contexts/constants';
import { draftEditors } from '../drafts/constants';
import {
  QueuedFileUploadsContext,
  useQueuedFileUploads,
  type QueuedFiles,
} from '../files/queuedFileUploads';
import { useToast } from '../../components/useToast';
import { createEntity, getBlueprintByCode } from './api';
import { EntityForm, type EntityFormHandle } from './components/EntityForm';
import { EntityPage } from './components/EntityPage';
import { entityQueryKeys } from './queryKeys';
import { invalidateEntitySearches } from './invalidateEntity';
import { attributeValueKinds } from './valueTypes';

export const CreateEntityPage = ({
  search,
}: {
  search: { blueprint?: string; locked?: boolean };
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const navigate = useNavigate({ from: '/entities/new' });
  const entityFormRef = useRef<EntityFormHandle>(null);
  // The chosen blueprint lives in the URL so a refresh reloads the same form
  // and can offer its saved draft.
  const blueprintCode = search.blueprint;
  const blueprint = useQuery({
    queryKey: entityQueryKeys.blueprintByCode(blueprintCode, undefined),
    queryFn: ({ signal }) =>
      getBlueprintByCode(blueprintCode!, undefined, signal),
    enabled: Boolean(blueprintCode),
  });
  // Another blueprint or version may not have the same file attributes.
  const queuedFiles = useQueuedFileUploads(
    blueprint.data &&
      `${blueprint.data.blueprint.id}:${blueprint.data.blueprint.version}`,
  );
  const toast = useToast();
  const contexts = useQuery({
    queryKey: contextQueryKeys.all(),
    queryFn: ({ signal }) => listContexts(signal),
  });
  const defaultContextId = contexts.data?.find(
    (context) => context.code === defaultContextCode,
  )?.id;
  const create = useMutation({
    mutationFn: async ({
      values,
      relationships,
      files,
    }: {
      values: Parameters<typeof createEntity>[0]['values'];
      relationships: { attribute_code: string; target_entity_ids: string[] }[];
      files: QueuedFiles;
    }) => {
      const resolved = blueprint.data;
      if (!resolved) throw new Error(t('entities.chooseBeforeCreate'));
      if (!defaultContextId)
        throw new Error(t('entities.defaultContextUnavailable'));
      const entity = await createEntity({
        blueprint: {
          code: resolved.blueprint.code,
          version: resolved.blueprint.version,
        },
        values: [
          ...values.map((value) => ({
            ...value,
            context_id: defaultContextId,
          })),
          ...relationships.flatMap((relationship) =>
            relationship.target_entity_ids.map((target_entity_id) => ({
              kind: attributeValueKinds.relationship,
              attribute_code: relationship.attribute_code,
              context_id: defaultContextId,
              target_entity_id,
            })),
          ),
        ],
      });
      // The entity exists now: forget the draft before the slower uploads so
      // leaving mid-upload cannot offer to create it again. A failed upload
      // is retried on the entity's page.
      entityFormRef.current?.clearDraft();
      void invalidateEntitySearches(client);
      const failedFiles = await queuedFiles.uploadQueued(
        files,
        entity.id,
        defaultContextId,
      );
      return { entity, failedFiles };
    },
    onSuccess: ({ entity, failedFiles }) => {
      if (failedFiles.length > 0)
        toast.show({
          message: t('files.uploadAfterCreateFailed', {
            count: failedFiles.length,
            files: failedFiles
              .map(({ filename, message }) =>
                message ? `${filename} (${message})` : filename,
              )
              .join(', '),
          }),
          severity: 'error',
        });
      void navigate({
        to: '/entities/$entityId',
        params: { entityId: entity.id },
      });
    },
  });
  return (
    <EntityPage
      blueprint={blueprint.data?.blueprint}
      title={t('entities.createEntity')}
    >
      <QueuedFileUploadsContext value={queuedFiles.queue}>
        <EntityForm
          blueprint={blueprint.data}
          contextId={defaultContextId}
          draft={
            blueprint.data && {
              editor: draftEditors.entityCreate,
              resource: [blueprint.data.blueprint.id],
              source: String(blueprint.data.blueprint.version),
            }
          }
          defaultContextId={defaultContextId}
          error={blueprint.error ?? create.error}
          isLoadingBlueprint={blueprint.isFetching || create.isPending}
          lockedBlueprint={search.locked}
          onLoadBlueprint={(code) =>
            void navigate({
              replace: true,
              search: { ...search, blueprint: code },
            })
          }
          onSubmit={({ values, relationships }) =>
            create.mutate({
              values,
              relationships,
              files: queuedFiles.queue.pending,
            })
          }
          ref={entityFormRef}
          submitLabel={
            blueprint.data
              ? t('entities.createEntity')
              : t('entities.loadBlueprint')
          }
        />
      </QueuedFileUploadsContext>
    </EntityPage>
  );
};
