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
  Radio,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { LoadMoreButton } from '../../components/LoadMoreButton';
import type { AttributeContext } from '../contexts/api';
import { getRelationshipTreeFacetChildren } from '../entities/api';
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
  singleSelect?: boolean;
  onHierarchyFieldChange: (field: string) => void;
  onSelectedIdsChange: (ids: string[]) => void;
};

export const RelationshipTreeFacet = (props: Props) => (
  <RelationshipTreeFacetContent
    key={[
      props.blueprint,
      props.version ?? '',
      props.query ?? '',
      props.sourceField,
      props.hierarchyField ?? '',
      props.contextCode ?? '',
    ].join('|')}
    {...props}
  />
);

const RelationshipTreeFacetContent = ({
  blueprint,
  version,
  query,
  sourceField,
  hierarchyFields,
  hierarchyField,
  contexts,
  contextCode,
  selectedIds,
  singleSelect = false,
  onHierarchyFieldChange,
  onSelectedIdsChange,
}: Props) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [cursors, setCursors] = useState<Map<string, (string | null)[]>>(
    new Map([['root', [null]]]),
  );
  const selected = new Set(selectedIds);
  const contextId = contexts.find(
    (context) => context.code === contextCode,
  )?.id;
  const pages = [...cursors.entries()].flatMap(([parentKey, pageCursors]) =>
    pageCursors.map((cursor) => {
      const parentId = parentKey === 'root' ? undefined : parentKey;
      return {
        parentId,
        cursor,
        selectedTargetIds:
          parentId === undefined && cursor === null ? selectedIds : [],
      };
    }),
  );
  const results = useQueries({
    queries: pages.map(({ parentId, cursor, selectedTargetIds }) => ({
      queryKey: entityQueryKeys.relationshipTreeFacetChildren(
        blueprint,
        version,
        query,
        sourceField,
        hierarchyField ?? '',
        contextCode ?? '',
        parentId,
        cursor,
        selectedTargetIds,
      ),
      queryFn: ({ signal }) =>
        getRelationshipTreeFacetChildren(
          {
            blueprint: {
              code: blueprint,
              ...(version === undefined ? {} : { version }),
            },
            ...(query ? { query } : {}),
            source_relationship_field: sourceField,
            ...(hierarchyField ? { hierarchy_field: hierarchyField } : {}),
            context_id: contextId!,
            ...(parentId === undefined ? {} : { parent_id: parentId }),
            cursor,
            selected_target_ids: selectedTargetIds,
          },
          signal,
        ),
      enabled: Boolean(contextId),
    })),
  });
  const byParent = new Map<string, RelationshipTreeFacetItem[]>();
  const selectedLabels = new Map<string, string>();
  const nextCursorByParent = new Map<string, string | null>();
  pages.forEach(({ parentId }, index) => {
    const page = results[index]?.data;
    if (!page) return;
    page.selected_items.forEach((item) =>
      selectedLabels.set(item.id, item.display),
    );
    const key = parentId ?? 'root';
    const items = byParent.get(key) ?? [];
    items.push(...page.items.filter((item) => !selected.has(item.id)));
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
  const isLoadingMore = (parentId: string | undefined) => {
    for (let index = pages.length - 1; index >= 0; index -= 1) {
      if (pages[index].parentId === parentId)
        return results[index]?.isFetching ?? false;
    }
    return false;
  };
  const toggleSelected = (id: string) => {
    if (singleSelect) {
      onSelectedIdsChange(selected.has(id) ? [] : [id]);
      return;
    }
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
                  open
                    ? t('explorer.collapse', { item: item.display })
                    : t('explorer.expand', { item: item.display })
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
              singleSelect ? (
                <Radio
                  checked={selected.has(item.id)}
                  onChange={() => toggleSelected(item.id)}
                  size="small"
                />
              ) : (
                <Checkbox
                  checked={selected.has(item.id)}
                  onChange={() => toggleSelected(item.id)}
                  size="small"
                />
              )
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
                <LoadMoreButton
                  isLoading={isLoadingMore(item.id)}
                  onLoadMore={() => loadMore(item.id)}
                />
              </ListItem>
            )}
          </List>
        )}
      </Box>
    );
  };

  return (
    <Stack spacing={2}>
      {selectedIds.length > 0 && (
        <Stack spacing={0.5}>
          <Typography variant="subtitle2">
            {t('entities.selectedRelationships')}
          </Typography>
          <List dense disablePadding>
            {selectedIds.map((id) => (
              <ListItem dense disableGutters key={id}>
                <FormControlLabel
                  control={
                    singleSelect ? (
                      <Radio
                        checked
                        onChange={() => toggleSelected(id)}
                        size="small"
                      />
                    ) : (
                      <Checkbox
                        checked
                        onChange={() => toggleSelected(id)}
                        size="small"
                      />
                    )
                  }
                  label={selectedLabels.get(id) ?? id}
                  sx={{ m: 0 }}
                />
              </ListItem>
            ))}
          </List>
        </Stack>
      )}
      <List dense disablePadding>
        {(byParent.get('root') ?? []).map((item) =>
          renderNode(item, new Set()),
        )}
        {nextCursorByParent.get('root') && (
          <ListItem disableGutters>
            <LoadMoreButton
              isLoading={isLoadingMore(undefined)}
              onLoadMore={() => loadMore(undefined)}
            />
          </ListItem>
        )}
      </List>
      {hierarchyFields.length > 0 && (
        <Accordion disableGutters elevation={0}>
          <AccordionSummary expandIcon={<ExpandMoreIcon />}>
            <Typography variant="body2">{t('explorer.treeOptions')}</Typography>
          </AccordionSummary>
          <AccordionDetails>
            <TextField
              fullWidth
              label={t('explorer.buildTreeUsing')}
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
          </AccordionDetails>
        </Accordion>
      )}
    </Stack>
  );
};
