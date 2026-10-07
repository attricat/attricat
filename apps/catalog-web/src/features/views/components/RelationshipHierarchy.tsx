import { useQueries } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Breadcrumbs, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { getEntityHierarchy } from '../../entities/api';
import { entityQueryKeys } from '../../entities/queryKeys';
import type { ViewComponentDefinition } from './componentTypes';
import {
  HIERARCHY_FONT_SIZE,
  HIERARCHY_PARENT_FIELD_PROP,
  VIEW_COMPONENT_IDS,
  VIEW_COMPONENT_VERSION,
} from '../constants';

export const RelationshipHierarchy = ({
  attribute,
  component,
  contextId,
  entityId,
  value,
}: {
  attribute: { code: string };
  component?: { props: Record<string, unknown> } | null;
  contextId?: string;
  entityId?: string;
  value: unknown;
}) => {
  const { t } = useTranslation();
  const parentField = component?.props[HIERARCHY_PARENT_FIELD_PROP];
  const targetIds =
    typeof parentField === 'string'
      ? [
          ...new Set(
            (value as { items?: { id?: string }[] } | undefined)?.items
              ?.map((item) => item.id)
              .filter((id): id is string => Boolean(id)) ?? [],
          ),
        ]
      : entityId
        ? [entityId]
        : [];
  const hierarchyField =
    typeof parentField === 'string' ? parentField : attribute.code;
  const hierarchies = useQueries({
    queries: targetIds.map((targetId) => ({
      queryKey: entityQueryKeys.hierarchy(
        targetId,
        contextId ?? '',
        hierarchyField,
      ),
      queryFn: () => getEntityHierarchy(targetId, contextId!, hierarchyField),
      enabled: Boolean(contextId),
    })),
  });
  if (!contextId)
    return (
      <Typography color="text.secondary">{t('views.noHierarchy')}</Typography>
    );
  if (hierarchies.some((hierarchy) => hierarchy.isPending))
    return <Typography>{t('views.loadingHierarchy')}</Typography>;
  const failedHierarchy = hierarchies.find((hierarchy) => hierarchy.isError);
  if (failedHierarchy)
    return (
      <Typography color="error">{failedHierarchy.error.message}</Typography>
    );
  const paths = hierarchies.flatMap(
    (hierarchy) => hierarchy.data?.paths ?? [hierarchy.data?.items ?? []],
  );
  if (!paths.length)
    return (
      <Typography color="text.secondary">{t('views.noHierarchy')}</Typography>
    );

  return (
    <Stack spacing={0.5}>
      {paths.map((items) => (
        <Breadcrumbs
          aria-label={t('views.hierarchy')}
          key={items.map((item) => item.id).join(':')}
          sx={{ fontSize: HIERARCHY_FONT_SIZE }}
        >
          {items.map((item) => (
            <Link
              key={item.id}
              params={{ entityId: item.id }}
              to="/entities/$entityId"
            >
              {item.display || item.id}
            </Link>
          ))}
        </Breadcrumbs>
      ))}
    </Stack>
  );
};

export const relationshipHierarchyComponent = {
  id: VIEW_COMPONENT_IDS.relationshipHierarchy,
  version: VIEW_COMPONENT_VERSION,
  capabilities: ['display'],
  placements: ['relationship_list'],
  value_types: ['relationship'],
  allowed_props: [HIERARCHY_PARENT_FIELD_PROP],
  valueRenderer: RelationshipHierarchy,
  showsWhileEditing: true,
} satisfies ViewComponentDefinition;
