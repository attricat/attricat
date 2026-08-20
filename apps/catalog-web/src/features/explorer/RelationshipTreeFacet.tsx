import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import ChevronRightIcon from '@mui/icons-material/ChevronRight';
import { useQueries } from '@tanstack/react-query';
import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Box,
  Checkbox,
  FormControlLabel,
  List,
  ListItem,
  MenuItem,
  TextField,
  Typography,
} from '@mui/material';
import { useEffect, useState } from 'react';
import {
  getRelationshipTreeFacetChildren,
  type AttributeContext,
} from '../entities/api';
import { entityQueryKeys } from '../entities/query-keys';

export type RelationshipTreeFacetItem = {
  id: string;
  display: string;
  count: number;
  has_children: boolean;
};

type Props = {
  blueprint: string;
  version?: number;
  query?: string;
  sourceField: string;
  hierarchyFields: string[];
  hierarchyField?: string;
  contexts: AttributeContext[];
  contextCode?: string;
  selectedIds: string[];
  onHierarchyFieldChange: (field: string) => void;
  onContextChange: (contextCode: string) => void;
  onSelectedIdsChange: (ids: string[]) => void;
};

export const RelationshipTreeFacet = ({
  blueprint,
  version,
  query,
  sourceField,
  hierarchyFields,
  hierarchyField,
  contexts,
  contextCode,
  selectedIds,
  onHierarchyFieldChange,
  onContextChange,
  onSelectedIdsChange,
}: Props) => {
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [cursors, setCursors] = useState<Map<string, (string | null)[]>>(
    new Map([['root', [null]]]),
  );
  const facetKey = [
    blueprint,
    version ?? '',
    query ?? '',
    sourceField,
    hierarchyField ?? '',
    contextCode ?? '',
  ].join('|');

  useEffect(() => {
    setExpanded(new Set());
    setCursors(new Map([['root', [null]]]));
  }, [facetKey]);
  const selected = new Set(selectedIds);
  const pages = [...cursors.entries()].flatMap(([parentKey, pageCursors]) =>
    pageCursors.map((cursor) => ({
      parentId: parentKey === 'root' ? undefined : parentKey,
      cursor,
    })),
  );
  const results = useQueries({
    queries: pages.map(({ parentId, cursor }) => ({
      queryKey: entityQueryKeys.relationshipTreeFacetChildren(
        blueprint,
        version,
        query,
        sourceField,
        hierarchyField ?? '',
        contextCode ?? '',
        parentId,
        cursor,
      ),
      queryFn: () =>
        getRelationshipTreeFacetChildren({
          blueprint: {
            code: blueprint,
            ...(version === undefined ? {} : { version }),
          },
          ...(query ? { query } : {}),
          source_relationship_field: sourceField,
          hierarchy_field: hierarchyField!,
          context_id: contexts.find((context) => context.code === contextCode)!
            .id,
          ...(parentId === undefined ? {} : { parent_id: parentId }),
          cursor,
        }),
      enabled: Boolean(hierarchyField && contextCode),
    })),
  });
  const byParent = new Map<string, RelationshipTreeFacetItem[]>();
  const nextCursorByParent = new Map<string, string | null>();
  pages.forEach(({ parentId }, index) => {
    const page = results[index]?.data;
    if (!page) return;
    const key = parentId ?? 'root';
    const items = byParent.get(key) ?? [];
    items.push(...page.items);
    byParent.set(key, items);
    nextCursorByParent.set(key, page.next_cursor);
  });
  const loadMore = (parentId: string | undefined) => {
    const key = parentId ?? 'root';
    const cursor = nextCursorByParent.get(key);
    if (!cursor) return;
    setCursors((current) => {
      const next = new Map(current);
      next.set(key, [...(next.get(key) ?? []), cursor]);
      return next;
    });
  };
  const toggleSelected = (id: string) => {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    onSelectedIdsChange([...next]);
  };
  const renderNode = (
    item: RelationshipTreeFacetItem,
    ancestry: Set<string>,
  ) => {
    const children = byParent.get(item.id) ?? [];
    const canExpand = item.has_children && !ancestry.has(item.id);
    const open = expanded.has(item.id);
    return (
      <Box key={item.id}>
        <ListItem dense disableGutters sx={{ minHeight: 32 }}>
          <Box sx={{ width: 28 }}>
            {canExpand && (
              <Box
                aria-label={
                  open ? `Collapse ${item.display}` : `Expand ${item.display}`
                }
                component="button"
                onClick={() => {
                  setExpanded((current) => {
                    const next = new Set(current);
                    if (next.has(item.id)) next.delete(item.id);
                    else {
                      next.add(item.id);
                      setCursors((current) => {
                        if (current.has(item.id)) return current;
                        return new Map(current).set(item.id, [null]);
                      });
                    }
                    return next;
                  });
                }}
                sx={{
                  alignItems: 'center',
                  background: 'none',
                  border: 0,
                  color: 'inherit',
                  cursor: 'pointer',
                  display: 'flex',
                  p: 0,
                }}
                type="button"
              >
                {open ? (
                  <ExpandMoreIcon fontSize="small" />
                ) : (
                  <ChevronRightIcon fontSize="small" />
                )}
              </Box>
            )}
          </Box>
          <FormControlLabel
            control={
              <Checkbox
                checked={selected.has(item.id)}
                onChange={() => toggleSelected(item.id)}
                size="small"
              />
            }
            label={`${item.display} (${item.count})`}
            sx={{ m: 0 }}
          />
        </ListItem>
        {canExpand && open && (
          <List dense disablePadding sx={{ pl: 3 }}>
            {children.map((child) =>
              renderNode(child, new Set(ancestry).add(item.id)),
            )}
            {nextCursorByParent.get(item.id) && (
              <ListItem disableGutters>
                <Box
                  component="button"
                  onClick={() => loadMore(item.id)}
                  type="button"
                >
                  Load more
                </Box>
              </ListItem>
            )}
          </List>
        )}
      </Box>
    );
  };

  return (
    <>
      <List dense disablePadding>
        {(byParent.get('root') ?? []).map((item) =>
          renderNode(item, new Set()),
        )}
        {nextCursorByParent.get('root') && (
          <ListItem disableGutters>
            <Box
              component="button"
              onClick={() => loadMore(undefined)}
              type="button"
            >
              Load more
            </Box>
          </ListItem>
        )}
      </List>
      <Accordion disableGutters elevation={0} sx={{ mt: 1 }}>
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography variant="body2">Tree options</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <TextField
            fullWidth
            label="Build tree using"
            onChange={(event) => onHierarchyFieldChange(event.target.value)}
            select
            size="small"
            value={hierarchyField ?? ''}
          >
            {hierarchyFields.map((field) => (
              <MenuItem key={field} value={field}>
                {field}
              </MenuItem>
            ))}
          </TextField>
          <TextField
            fullWidth
            label="Context"
            onChange={(event) => onContextChange(event.target.value)}
            select
            size="small"
            sx={{ mt: 2 }}
            value={contextCode ?? ''}
          >
            {contexts.map((context) => (
              <MenuItem key={context.id} value={context.code}>
                {context.code === 'default' ? 'Default' : context.code}
              </MenuItem>
            ))}
          </TextField>
        </AccordionDetails>
      </Accordion>
    </>
  );
};
