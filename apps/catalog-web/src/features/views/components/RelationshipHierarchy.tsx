import { useQueries } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Breadcrumbs, Stack, Typography } from '@mui/material';
import { getEntityHierarchy } from '../../entities/api';
import { entityQueryKeys } from '../../entities/query-keys';
import type { ViewComponentDefinition } from './component-types';

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
  const parentField = component?.props.parent_field;
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
  if (hierarchies.some((hierarchy) => hierarchy.isPending))
    return <Typography>Loading hierarchy...</Typography>;
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
      <Typography color="text.secondary">No hierarchy available.</Typography>
    );

  return (
    <Stack spacing={0.5}>
      {paths.map((items) => (
        <Breadcrumbs
          aria-label="Hierarchy"
          key={items.map((item) => item.id).join(':')}
          sx={{ fontSize: '0.875rem' }}
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
  id: 'catalog.relationship_hierarchy',
  version: 1,
  capabilities: ['display'],
  placements: ['relationship_list'],
  value_types: ['relationship'],
  allowed_props: ['parent_field'],
  valueRenderer: RelationshipHierarchy,
} satisfies ViewComponentDefinition;
