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
import { errorMessage } from '../../api/errors';

type Props = {
  blueprint: string;
  /** Labels the blueprint's own tree in the field picker. */
  blueprintName: string;
  contextId: string | undefined;
  /** The field Show opens the rail on. */
  defaultField: BrowseByField | undefined;
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
  defaultField,
  field,
  fields,
  query,
  selectedIds,
  version,
  onFieldChange,
  onSelect,
}: Props) => {
  const { t } = useTranslation();
  if (!fields.length || !defaultField) return null;
  if (!field) {
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

type LevelRequest = Omit<RelationshipTreeFacetChildrenRequest, 'cursor'>;

/** One level of the tree, page by page. */
const useLevelChildren = (levelRequest: LevelRequest, enabled = true) =>
  useInfiniteQuery({
    enabled,
    queryKey: recordQueryKeys.relationshipTreeFacetChildren(levelRequest),
    queryFn: ({ pageParam, signal }) =>
      getRelationshipTreeFacetChildren(
        { ...levelRequest, cursor: pageParam },
        signal,
      ),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.next_cursor,
  });

const pathKey = (ids: string[]) => ids.join('/');

/**
 * Every level asks for the nodes selected on mount, so each returns its node
 * on the way to them even past its first page, and the root level returns
 * the root-first paths to them. The selection is fixed for the tree's life,
 * so clicks never reload its levels.
 */
const BrowseTree = ({ request, ...props }: TreeProps) => {
  const [initialSelection] = useState(() => [...props.selected]);
  const selectionRequest = initialSelection.length
    ? { ...request, selected_target_ids: initialSelection }
    : request;
  const selectionRoot = useLevelChildren(selectionRequest);
  // A stale selection fails the request; the tree then opens collapsed.
  const stale = selectionRoot.isError && initialSelection.length > 0;
  const plainRoot = useLevelChildren(request, stale);
  const treeRequest = stale ? request : selectionRequest;
  const paths = (!stale && selectionRoot.data?.pages[0]?.selected_paths) || [];
  // Open each branch above a selected node.
  const selectedBranches = new Set(
    paths.flatMap((path) =>
      path.slice(0, -1).map((_, index) => pathKey(path.slice(0, index + 1))),
    ),
  );
  // What the user opened or closed, over the branches leading to the selection.
  const [toggled, setToggled] = useState<Map<string, boolean>>(new Map());
  if (selectionRoot.isPending) return <LevelLoading depth={0} />;
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
      level={stale ? plainRoot : selectionRoot}
      onToggle={toggle}
      request={treeRequest}
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
  onToggle: (path: string) => void;
};

/** A level below the root, loaded when its parent opens. */
const ChildLevel = (props: LevelProps) => (
  <BrowseLevel
    {...props}
    level={useLevelChildren({
      ...props.request,
      parent_id: props.ancestry.at(-1),
    })}
  />
);

const BrowseLevel = ({
  ancestry,
  expanded,
  level: children,
  request,
  selected,
  onSelect,
  onToggle,
}: LevelProps & { level: ReturnType<typeof useLevelChildren> }) => {
  const { i18n, t } = useTranslation();
  const indent = ancestry.length * browseRailIndent;

  if (children.isPending) return <LevelLoading depth={ancestry.length} />;
  if (children.isError)
    return (
      <Typography color="error" sx={{ pl: indent + 1, py: 1 }} variant="body2">
        {errorMessage(children.error)}
      </Typography>
    );
  // Pages arrive in ID order; show each level alphabetically. A node on the
  // way to the selection returns again on the page that holds it.
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
      {open && <ChildLevel {...props} ancestry={[...ancestry, item.id]} />}
    </>
  );
};
