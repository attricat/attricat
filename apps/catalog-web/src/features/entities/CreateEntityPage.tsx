import { useMutation, useQuery } from '@tanstack/react-query';
import { useEffect } from 'react';
import { useNavigate } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { createEntity, getBlueprintByCode, listContexts } from './api';
import { EntityForm } from './components/EntityForm';
import { EntityPage } from './components/EntityPage';
import { attributeValueKinds } from './value-types';
import { entityQueryKeys } from './query-keys';

export const CreateEntityPage = ({
  search,
}: {
  search: { blueprint?: string; locked?: boolean };
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate({ from: '/entities/new' });
  const blueprint = useMutation({
    mutationFn: ({ code, version }: { code: string; version?: number }) =>
      getBlueprintByCode(code, version),
  });
  useEffect(() => {
    if (
      search.locked &&
      search.blueprint &&
      !blueprint.data &&
      !blueprint.isPending
    ) {
      blueprint.mutate({ code: search.blueprint });
    }
  }, [blueprint, search.blueprint, search.locked]);
  const contexts = useQuery({
    queryKey: entityQueryKeys.contexts(),
    queryFn: listContexts,
  });
  const defaultContextId = contexts.data?.find(
    (context) => context.code === 'default',
  )?.id;
  const create = useMutation({
    mutationFn: ({
      values,
      relationships,
    }: {
      values: Parameters<typeof createEntity>[0]['values'];
      relationships: { attribute_code: string; target_entity_ids: string[] }[];
    }) => {
      const resolved = blueprint.data;
      if (!resolved) throw new Error(t('entities.chooseBeforeCreate'));
      if (!defaultContextId)
        throw new Error(t('entities.defaultContextUnavailable'));
      return createEntity({
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
    },
    onSuccess: (entity) => {
      void navigate({
        to: '/entities/$entityId',
        params: { entityId: entity.id },
      });
    },
  });
  return (
    <EntityPage title={t('entities.createEntity')}>
      <EntityForm
        blueprint={blueprint.data}
        contextId={defaultContextId}
        defaultContextId={defaultContextId}
        error={blueprint.error ?? create.error}
        isLoadingBlueprint={blueprint.isPending || create.isPending}
        lockedBlueprint={search.locked}
        onLoadBlueprint={(code, version) => blueprint.mutate({ code, version })}
        onSubmit={({ values, relationships }) =>
          create.mutate({ values, relationships })
        }
        submitLabel={
          blueprint.data
            ? t('entities.createEntity')
            : t('entities.loadBlueprint')
        }
      />
    </EntityPage>
  );
};
