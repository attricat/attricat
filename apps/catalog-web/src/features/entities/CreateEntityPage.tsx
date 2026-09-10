import { useMutation, useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { listContexts } from '../contexts/api';
import { contextQueryKeys } from '../contexts/query-keys';
import { defaultContextCode } from '../contexts/constants';
import { createEntity, getBlueprintByCode } from './api';
import { EntityForm } from './components/EntityForm';
import { EntityPage } from './components/EntityPage';
import { entityQueryKeys } from './query-keys';
import { attributeValueKinds } from './value-types';

export const CreateEntityPage = ({
  search,
}: {
  search: { blueprint?: string; locked?: boolean };
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate({ from: '/entities/new' });
  const [selectedBlueprint, setSelectedBlueprint] = useState<{
    code: string;
    version?: number;
  }>();
  const blueprintSelection =
    search.locked && search.blueprint
      ? { code: search.blueprint }
      : selectedBlueprint;
  const blueprint = useQuery({
    queryKey: entityQueryKeys.blueprintByCode(
      blueprintSelection?.code,
      blueprintSelection?.version,
    ),
    queryFn: ({ signal }) =>
      getBlueprintByCode(
        blueprintSelection!.code,
        blueprintSelection!.version,
        signal,
      ),
    enabled: Boolean(blueprintSelection),
  });
  const contexts = useQuery({
    queryKey: contextQueryKeys.all(),
    queryFn: ({ signal }) => listContexts(signal),
  });
  const defaultContextId = contexts.data?.find(
    (context) => context.code === defaultContextCode,
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
        isLoadingBlueprint={blueprint.isFetching || create.isPending}
        lockedBlueprint={search.locked}
        onLoadBlueprint={(code, version) =>
          setSelectedBlueprint({ code, version })
        }
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
