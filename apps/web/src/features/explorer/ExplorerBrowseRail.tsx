import {
  Box,
  Button,
  CircularProgress,
  IconButton,
  List,
  ListItem,
  ListItemButton,
  ListItemText,
  MenuItem,
  Paper,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { useInfiniteQuery } from '@tanstack/react-query';
import {
  ChevronDownIcon,
  ChevronRightIcon,
  PanelLeftCloseIcon,
  PanelLeftOpenIcon,
} from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import {
  getRelationshipTreeFacetChildren,
  type RelationshipTreeFacetChild,
  type RelationshipTreeFacetChildrenRequest,
} from '../records/api';
import { recordQueryKeys } from '../records/queryKeys';
import { attributeLabel } from '../records/recordDisplay';
import type { BrowseByField } from './browseBy';
import {
  browseRailIndent,
  browseRailToggleWidth,
  browseRailWidth,
  resultsTableHeight,
} from './constants';
import type { BrowseBy } from './search';

type Props = {
  blueprint: string;
  contextId: string | undefined;
  field: BrowseByField | undefined;
  fields: BrowseByField[];
  query: string | undefined;
  selectedIds: string[];
  version: number | undefined;
  onFieldChange: (browseBy: BrowseBy) => void;
  onSelect: (selectedIds: string[]) => void;
};

/**
 * Lists the targets of one relationship field, as a tree when the target
 * blueprint has a hierarchy. Choosing a target filters the results to records
 * assigned to it or, in a tree, to any of its descendants.
 */
export const ExplorerBrowseRail = ({
  blueprint,
  contextId,
  field,
  fields,
  query,
  selectedIds,
  version,
  onFieldChange,
  onSelect,
}: Props) => {
  const { t } = useTranslation();
  if (!fields.length) return null;
  if (!field) {
    const defaultField =
      fields.find((candidate) => candidate.hierarchyField) ?? fields[0];
    return (
      <Box sx={{ flexShrink: 0, mt: 3 }}>
        <Tooltip placement="right" title={t('explorer.browse.show')}>
          <IconButton
            aria-label={t('explorer.browse.show')}
            onClick={() => onFieldChange(defaultField.attribute.code)}
          >
            <PanelLeftOpenIcon />
          </IconButton>
        </Tooltip>
      </Box>
    );
  }
  const fieldLabel = attributeLabel(field.attribute);
  const request: Omit<RelationshipTreeFacetChildrenRequest, 'cursor'> = {
    blueprint: {
      code: blueprint,
      ...(version === undefined ? {} : { version }),
    },
    ...(query ? { query } : {}),
    source_relationship_field: field.attribute.code,
    ...(field.hierarchyField ? { hierarchy_field: field.hierarchyField } : {}),
    context_id: contextId ?? '',
  };
  const selected = new Set(selectedIds);

  return (
    <Paper
      aria-label={t('explorer.browse.label', { field: fieldLabel })}
      component="aside"
      sx={{
        display: 'flex',
        flexDirection: 'column',
        flexShrink: 0,
        height: resultsTableHeight.md,
        mt: 3,
        width: browseRailWidth,
      }}
    >
      <Box
        sx={{
          alignItems: 'center',
          borderBottom: 1,
          borderColor: 'divider',
          display: 'flex',
          gap: 1,
          p: 1,
        }}
      >
        <TextField
          fullWidth
          label={t('explorer.browse.by')}
          onChange={(event) => onFieldChange(event.target.value)}
          select
          size="small"
          value={field.attribute.code}
        >
          {fields.map((candidate) => (
            <MenuItem
              key={candidate.attribute.code}
              value={candidate.attribute.code}
            >
              {attributeLabel(candidate.attribute)}
            </MenuItem>
          ))}
        </TextField>
        <Tooltip title={t('explorer.browse.hide')}>
          <IconButton
            aria-label={t('explorer.browse.hide')}
            onClick={() => onFieldChange(false)}
          >
            <PanelLeftCloseIcon />
          </IconButton>
        </Tooltip>
      </Box>
      <Box sx={{ flexGrow: 1, overflowY: 'auto', p: 1 }}>
        <List dense disablePadding>
          <ListItem disablePadding>
            <ListItemButton
              aria-current={selected.size === 0 || undefined}
              onClick={() => onSelect([])}
              selected={selected.size === 0}
              sx={{ borderRadius: 1 }}
            >
              <ListItemText primary={t('explorer.browse.all')} />
            </ListItemButton>
          </ListItem>
        </List>
        {contextId !== undefined && (
          <BrowseTree
            // A new field, search or context starts with a collapsed tree.
            key={JSON.stringify(request)}
            onSelect={onSelect}
            request={request}
            selected={selected}
          />
        )}
      </Box>
    </Paper>
  );
};

type TreeProps = {
  request: Omit<RelationshipTreeFacetChildrenRequest, 'cursor'>;
  selected: Set<string>;
  onSelect: (selectedIds: string[]) => void;
};

const BrowseTree = (props: TreeProps) => {
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const toggle = (path: string) =>
    setExpanded((current) => {
      const next = new Set(current);
      if (!next.delete(path)) next.add(path);
      return next;
    });
  return (
    <BrowseLevel
      {...props}
      ancestry={[]}
      expanded={expanded}
      onToggle={toggle}
    />
  );
};

type LevelProps = TreeProps & {
  /** Target IDs from the root to this level's parent. */
  ancestry: string[];
  expanded: Set<string>;
  onToggle: (path: string) => void;
};

const BrowseLevel = ({
  ancestry,
  expanded,
  request,
  selected,
  onSelect,
  onToggle,
}: LevelProps) => {
  const { i18n, t } = useTranslation();
  const parentId = ancestry.at(-1);
  const levelRequest = {
    ...request,
    ...(parentId === undefined ? {} : { parent_id: parentId }),
  };
  const children = useInfiniteQuery({
    queryKey: recordQueryKeys.relationshipTreeFacetChildren(levelRequest),
    queryFn: ({ pageParam, signal }) =>
      getRelationshipTreeFacetChildren(
        { ...levelRequest, cursor: pageParam },
        signal,
      ),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.next_cursor,
  });
  const indent = ancestry.length * browseRailIndent;

  if (children.isPending)
    return (
      <Box sx={{ pl: indent + 1, py: 1 }}>
        <CircularProgress
          aria-label={t('common.loading')}
          size={compactIconSize}
        />
      </Box>
    );
  if (children.isError)
    return (
      <Typography color="error" sx={{ pl: indent + 1, py: 1 }} variant="body2">
        {children.error.message}
      </Typography>
    );
  // Pages arrive in ID order; show each level alphabetically.
  const items = children.data.pages
    .flatMap((page) => page.items)
    .sort((left, right) =>
      left.display.localeCompare(right.display, i18n.language),
    );
  if (!items.length && ancestry.length === 0)
    return (
      <Typography color="text.secondary" sx={{ px: 1, py: 1 }} variant="body2">
        {t('explorer.browse.empty')}
      </Typography>
    );

  return (
    <List dense disablePadding>
      {items.map((item) => (
        <BrowseNode
          ancestry={ancestry}
          expanded={expanded}
          item={item}
          key={item.id}
          onSelect={onSelect}
          onToggle={onToggle}
          request={request}
          selected={selected}
        />
      ))}
      {children.hasNextPage && (
        <ListItem disablePadding sx={{ pl: indent + 4 }}>
          <Button
            disabled={children.isFetchingNextPage}
            onClick={() => void children.fetchNextPage()}
            size="small"
          >
            {t('common.loadMore')}
          </Button>
        </ListItem>
      )}
    </List>
  );
};

const BrowseNode = ({
  ancestry,
  item,
  ...props
}: LevelProps & { item: RelationshipTreeFacetChild }) => {
  const { t } = useTranslation();
  const { expanded, selected, onSelect, onToggle } = props;
  const path = [...ancestry, item.id].join('/');
  // An acyclic hierarchy is never expected to loop; stop if it does.
  const canExpand = item.has_children && !ancestry.includes(item.id);
  const open = canExpand && expanded.has(path);
  const isSelected = selected.has(item.id);
  const label = item.display || item.id;
  return (
    <>
      <ListItem disablePadding sx={{ pl: ancestry.length * browseRailIndent }}>
        {canExpand ? (
          <IconButton
            aria-expanded={open}
            aria-label={t(
              open ? 'explorer.browse.collapse' : 'explorer.browse.expand',
              { item: label },
            )}
            onClick={() => onToggle(path)}
            size="small"
          >
            {open ? (
              <ChevronDownIcon size={compactIconSize} />
            ) : (
              <ChevronRightIcon size={compactIconSize} />
            )}
          </IconButton>
        ) : (
          <Box
            aria-hidden
            sx={{ flexShrink: 0, width: browseRailToggleWidth }}
          />
        )}
        <ListItemButton
          aria-current={isSelected || undefined}
          onClick={() =>
            onSelect(isSelected && selected.size === 1 ? [] : [item.id])
          }
          selected={isSelected}
          sx={{ borderRadius: 1, gap: 1, minWidth: 0 }}
        >
          <ListItemText
            primary={label}
            slotProps={{ primary: { noWrap: true, title: label } }}
          />
          <Typography color="text.secondary" variant="caption">
            {item.count}
          </Typography>
        </ListItemButton>
      </ListItem>
      {open && <BrowseLevel {...props} ancestry={[...ancestry, item.id]} />}
    </>
  );
};
