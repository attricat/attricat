import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import ChevronRightIcon from '@mui/icons-material/ChevronRight';
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
import { useState } from 'react';
import type { AttributeContext } from '../entities/api';

export type RelationshipTreeFacetItem = {
  id: string;
  parent_ids: string[];
  display: string;
  count: number;
};

type Props = {
  items?: RelationshipTreeFacetItem[];
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
  items,
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
  const selected = new Set(selectedIds);
  const byParent = new Map<string, RelationshipTreeFacetItem[]>();
  const itemIds = new Set(items?.map((item) => item.id));
  for (const item of items ?? []) {
    for (const parentId of item.parent_ids) {
      const children = byParent.get(parentId) ?? [];
      children.push(item);
      byParent.set(parentId, children);
    }
  }
  const roots = (items ?? []).filter(
    (item) => !item.parent_ids.some((parentId) => itemIds.has(parentId)),
  );
  const visibleRoots = roots.length ? roots : (items ?? []);
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
    const canExpand = children.length > 0 && !ancestry.has(item.id);
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
                    else next.add(item.id);
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
          </List>
        )}
      </Box>
    );
  };

  return (
    <>
      {items && (
        <List dense disablePadding>
          {visibleRoots.map((item) => renderNode(item, new Set()))}
        </List>
      )}
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
