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
import { ApiRequestError } from '../../api/request';
import { createRecord, getBlueprintByCode } from './api';
import { RecordForm, type RecordFormHandle } from './components/RecordForm';
import { RecordPage } from './components/RecordPage';
import { recordQueryKeys } from './queryKeys';
import { invalidateRecordSearches } from './invalidateRecord';
import { attributeValueKinds } from './valueTypes';
import { invalidFileReferencesCode } from '../files/constants';

export const CreateRecordPage = ({
  search,
}: {
  search: { blueprint?: string; locked?: boolean };
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const navigate = useNavigate({ from: '/records/new' });
  const recordFormRef = useRef<RecordFormHandle>(null);
  // The chosen blueprint lives in the URL so a refresh reloads the same form
  // and can offer its saved draft.
  const blueprintCode = search.blueprint;
  const blueprint = useQuery({
    queryKey: recordQueryKeys.blueprintByCode(blueprintCode, undefined),
    queryFn: ({ signal }) =>
      getBlueprintByCode(blueprintCode!, undefined, signal),
    enabled: Boolean(blueprintCode),
  });
  // Another blueprint or version may not have the same file attributes.
  const queuedFiles = useQueuedFileUploads(
    blueprint.data &&
      `${blueprint.data.blueprint.id}:${blueprint.data.blueprint.version}`,
  );
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
      values: Parameters<typeof createRecord>[0]['values'];
      relationships: { attribute_code: string; target_record_ids: string[] }[];
      files: QueuedFiles;
    }) => {
      const resolved = blueprint.data;
      if (!resolved) throw new Error(t('records.chooseBeforeCreate'));
      if (!defaultContextId)
        throw new Error(t('records.defaultContextUnavailable'));
      // Files upload first, so a record whose schema requires a file is
      // created with it. If any fails, nothing is created and the queue
      // keeps each file to retry or remove.
      const staged = await queuedFiles.stageQueued(
        files,
        resolved.blueprint.id,
        defaultContextId,
      );
      if (staged.failed.length > 0)
        throw new Error(
          t('files.uploadBeforeCreateFailed', {
            count: staged.failed.length,
            files: staged.failed
              .map(({ filename, message }) =>
                message ? `${filename} (${message})` : filename,
              )
              .join(', '),
          }),
        );
      try {
        return await createRecord({
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
              relationship.target_record_ids.map((target_record_id) => ({
                kind: attributeValueKinds.relationship,
                attribute_code: relationship.attribute_code,
                context_id: defaultContextId,
                target_record_id,
              })),
            ),
          ],
          files: staged.staged,
        });
      } catch (error) {
        // Staged files that can no longer be claimed, such as after their
        // upload expired, upload again on the next attempt.
        if (
          error instanceof ApiRequestError &&
          error.code === invalidFileReferencesCode
        )
          queuedFiles.forgetStaged();
        throw error;
      }
    },
    onSuccess: (record) => {
      recordFormRef.current?.clearDraft();
      void invalidateRecordSearches(client);
      void navigate({
        to: '/records/$recordId',
        params: { recordId: record.id },
      });
    },
  });
  return (
    <RecordPage
      blueprint={blueprint.data?.blueprint}
      title={t('records.createRecord')}
    >
      <QueuedFileUploadsContext value={queuedFiles.queue}>
        <RecordForm
          blueprint={blueprint.data}
          contextId={defaultContextId}
          draft={
            blueprint.data && {
              editor: draftEditors.recordCreate,
              resource: [blueprint.data.blueprint.id],
              source: String(blueprint.data.blueprint.version),
            }
          }
          defaultContextId={defaultContextId}
          error={blueprint.error ?? create.error}
          // A background refresh must not disable the fields being filled in.
          isLoadingBlueprint={blueprint.isLoading || create.isPending}
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
          ref={recordFormRef}
          submitLabel={
            blueprint.data
              ? t('records.createRecord')
              : t('records.loadBlueprint')
          }
        />
      </QueuedFileUploadsContext>
    </RecordPage>
  );
};
