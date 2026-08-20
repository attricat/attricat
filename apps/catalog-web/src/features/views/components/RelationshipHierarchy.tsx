import { useQueries } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Breadcrumbs, Button, Menu, MenuItem, Typography } from '@mui/material';
import { useState } from 'react';
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
  const [selection, setSelection] = useState({ pathKey: '', index: 0 });
  const [pathMenuAnchor, setPathMenuAnchor] = useState<HTMLElement | null>(
    null,
  );
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
  const pathKey = `${targetIds.join(':')}:${contextId ?? ''}:${hierarchyField}`;
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
  const selectedPath =
    selection.pathKey === pathKey && selection.index < paths.length
      ? selection.index
      : 0;
  const items = paths[selectedPath] ?? [];
  if (!items.length)
    return (
      <Typography color="text.secondary">No hierarchy available.</Typography>
    );

  return (
    <>
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
        {paths.length > 1 && (
          <Button
            aria-controls={pathMenuAnchor ? 'hierarchy-paths' : undefined}
            aria-expanded={pathMenuAnchor ? 'true' : undefined}
            aria-haspopup="menu"
            onClick={(event) => setPathMenuAnchor(event.currentTarget)}
            size="small"
          >
            + {paths.length - 1} paths
          </Button>
        )}
      </Breadcrumbs>
      <Menu
        anchorEl={pathMenuAnchor}
        id="hierarchy-paths"
        onClose={() => setPathMenuAnchor(null)}
        open={Boolean(pathMenuAnchor)}
      >
        {paths.map((path, index) => (
          <MenuItem
            key={path.map((item) => item.id).join(':')}
            onClick={() => {
              setSelection({ pathKey, index });
              setPathMenuAnchor(null);
            }}
            selected={index === selectedPath}
          >
            {path.map((item) => item.display || item.id).join(' / ')}
          </MenuItem>
        ))}
      </Menu>
    </>
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
