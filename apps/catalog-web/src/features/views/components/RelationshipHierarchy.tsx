import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Breadcrumbs, Typography } from '@mui/material';
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
  const targetId =
    typeof parentField === 'string'
      ? (value as { items?: { id?: string }[] } | undefined)?.items?.[0]?.id
      : entityId;
  const hierarchyField = typeof parentField === 'string' ? parentField : attribute.code;
  const hierarchy = useQuery({
    queryKey: entityQueryKeys.hierarchy(
      targetId ?? '',
      contextId ?? '',
      hierarchyField,
    ),
    queryFn: () => getEntityHierarchy(targetId!, contextId!, hierarchyField),
    enabled: Boolean(targetId && contextId),
  });
  if (hierarchy.isPending) return <Typography>Loading hierarchy...</Typography>;
  if (hierarchy.isError)
    return <Typography color="error">{hierarchy.error.message}</Typography>;
  const items = hierarchy.data?.items ?? [];
  if (!items.length)
    return (
      <Typography color="text.secondary">No hierarchy available.</Typography>
    );

  return (
    <Breadcrumbs aria-label="Hierarchy">
      {items.map((item, index) =>
        index === items.length - 1 ? (
          <Typography color="text.primary" key={item.id}>
            {item.display || item.id}
          </Typography>
        ) : (
          <Link
            key={item.id}
            params={{ entityId: item.id }}
            to="/entities/$entityId"
          >
            {item.display || item.id}
          </Link>
        ),
      )}
    </Breadcrumbs>
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
