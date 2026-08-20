import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { Breadcrumbs, Button, Menu, MenuItem, Typography } from '@mui/material';
import { useEffect, useState } from 'react';
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
  const [selectedPath, setSelectedPath] = useState(0);
  const [pathMenuAnchor, setPathMenuAnchor] = useState<HTMLElement | null>(
    null,
  );
  const parentField = component?.props.parent_field;
  const targetId =
    typeof parentField === 'string'
      ? (value as { items?: { id?: string }[] } | undefined)?.items?.[0]?.id
      : entityId;
  const hierarchyField =
    typeof parentField === 'string' ? parentField : attribute.code;
  const hierarchy = useQuery({
    queryKey: entityQueryKeys.hierarchy(
      targetId ?? '',
      contextId ?? '',
      hierarchyField,
    ),
    queryFn: () => getEntityHierarchy(targetId!, contextId!, hierarchyField),
    enabled: Boolean(targetId && contextId),
  });
  useEffect(() => {
    setSelectedPath(0);
  }, [targetId, contextId, hierarchyField]);
  if (hierarchy.isPending) return <Typography>Loading hierarchy...</Typography>;
  if (hierarchy.isError)
    return <Typography color="error">{hierarchy.error.message}</Typography>;
  const paths = hierarchy.data?.paths ?? [];
  const items = paths[selectedPath] ?? hierarchy.data?.items ?? [];
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
              setSelectedPath(index);
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
