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
import { useInfiniteQuery, useQuery } from '@tanstack/react-query';
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
  /** Labels the blueprint's own tree in the field picker. */
  blueprintName: string;
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
  blueprintName,
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
  const label = (candidate: BrowseByField) =>
    candidate.ownTree ? blueprintName : attributeLabel(candidate.attribute);
  const fieldLabel = label(field);
  const request: Omit<RelationshipTreeFacetChildrenRequest, 'cursor'> = {
    blueprint: {
      code: blueprint,
      ...(version === undefined ? {} : { version }),
    },
    ...(query ? { query } : {}),
    ...(field.ownTree
      ? {}
      : { source_relationship_field: field.attribute.code }),
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
              {label(candidate)}
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

/**
 * Root-first paths to the nodes selected on mount, ending with each node. The
 * selection is fixed for the tree's life, so clicks never reload its levels.
 */
const useSelectedPaths = ({ request, selected }: TreeProps) => {
  const [initialSelection] = useState(() => [...selected]);
  const pathsRequest = { ...request, selected_target_ids: initialSelection };
  // A stale selection fails the request; the tree then just stays collapsed.
  const paths = useQuery({
    queryKey: recordQueryKeys.relationshipTreeFacetSelectedPaths(pathsRequest),
    queryFn: ({ signal }) =>
      getRelationshipTreeFacetChildren(
        { ...pathsRequest, cursor: null },
        signal,
      ),
    enabled: initialSelection.length > 0,
    select: (page) => page.selected_paths,
    staleTime: Infinity,
  });
  return {
    paths: paths.data ?? [],
    settled: initialSelection.length === 0 || !paths.isPending,
  };
};

const pathKey = (ids: string[]) => ids.join('/');

const BrowseTree = (props: TreeProps) => {
  const { paths, settled } = useSelectedPaths(props);
  // Open each branch above a selected node, and ask each level on the way
  // for its node in case it falls past the first page.
  const selectedBranches = new Set(
    paths.flatMap((path) =>
      path.slice(0, -1).map((_, index) => pathKey(path.slice(0, index + 1))),
    ),
  );
  const included = new Map<string, string[]>();
  for (const path of paths)
    path.forEach((id, index) => {
      const level = pathKey(path.slice(0, index));
      included.set(level, [...(included.get(level) ?? []), id]);
    });
  // What the user opened or closed, over the branches leading to the selection.
  const [toggled, setToggled] = useState<Map<string, boolean>>(new Map());
  if (!settled) return <LevelLoading depth={0} />;
  const expanded = new Set(
    [...selectedBranches, ...toggled.keys()].filter(
      (path) => toggled.get(path) ?? selectedBranches.has(path),
    ),
  );
  const toggle = (path: string) =>
    setToggled((current) =>
      new Map(current).set(
        path,
        !(current.get(path) ?? selectedBranches.has(path)),
      ),
    );
  return (
    <BrowseLevel
      {...props}
      ancestry={[]}
      expanded={expanded}
      included={included}
      onToggle={toggle}
    />
  );
};

const LevelLoading = ({ depth }: { depth: number }) => {
  const { t } = useTranslation();
  return (
    <Box sx={{ pl: depth * browseRailIndent + 1, py: 1 }}>
      <CircularProgress
        aria-label={t('common.loading')}
        size={compactIconSize}
      />
    </Box>
  );
};

type LevelProps = TreeProps & {
  /** Target IDs from the root to this level's parent. */
  ancestry: string[];
  expanded: Set<string>;
  /** Nodes each level returns even past its first page, by ancestry path. */
  included: Map<string, string[]>;
  onToggle: (path: string) => void;
};

const BrowseLevel = ({
  ancestry,
  expanded,
  included,
  request,
  selected,
  onSelect,
  onToggle,
}: LevelProps) => {
  const { i18n, t } = useTranslation();
  const parentId = ancestry.at(-1);
  const includeIds = included.get(pathKey(ancestry));
  const levelRequest = {
    ...request,
    ...(parentId === undefined ? {} : { parent_id: parentId }),
    ...(includeIds ? { include_ids: includeIds } : {}),
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

  if (children.isPending) return <LevelLoading depth={ancestry.length} />;
  if (children.isError)
    return (
      <Typography color="error" sx={{ pl: indent + 1, py: 1 }} variant="body2">
        {children.error.message}
      </Typography>
    );
  // Pages arrive in ID order; show each level alphabetically. An included
  // node returns again on the page that holds it.
  const items = [
    ...new Map(
      children.data.pages
        .flatMap((page) => page.items)
        .map((item) => [item.id, item]),
    ).values(),
  ].sort((left, right) =>
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
          included={included}
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
  const path = pathKey([...ancestry, item.id]);
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
