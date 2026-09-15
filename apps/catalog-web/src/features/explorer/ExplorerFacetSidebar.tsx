import {
  Box,
  Button,
  Chip,
  Link,
  MenuItem,
  List,
  ListItem,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useInfiniteQuery } from '@tanstack/react-query';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { Attribute, Blueprint } from '../entities/api';
import type { AttributeContext } from '../contexts/api';
import { defaultContextCode } from '../contexts/constants';
import { searchEntities } from '../entities/api';
import { entityQueryKeys } from '../entities/query-keys';
import { RelationshipSelectorDialog } from '../../components/RelationshipSelectorDialog';
import {
  RelationshipIcon,
  RelationshipPickerIcon,
} from '../../components/system-icons';
import { RelationshipSelectionPills } from '../entities/components/RelationshipSelectionPills';
import { useRelationshipSelectionLabels } from '../entities/components/useRelationshipSelectionLabels';
import {
  scrollRelationshipPickerToTop,
  useRecentlyPreviewedEntities,
} from '../entities/components/useRecentlyPreviewedEntities';
import { ExplorerFilterPicker } from './ExplorerFilterPicker';
import { LoadMoreButton } from '../../components/LoadMoreButton';
import {
  attributeLabel,
  dropdownOptionLabel,
  displayLabel,
} from '../entities/entity-display';
import type { RelationshipFilterAttribute } from './relationship-filter-types';
import type { AttributeFilter } from './search';

export type ExplorerRelationshipFacet = {
  selectedIds: string[];
  sourceRelationship: RelationshipFilterAttribute;
};

type Props = {
  blueprint: string;
  blueprints?: Blueprint[];
  contexts: AttributeContext[];
  contextCode: string;
  facets: ExplorerRelationshipFacet[];
  attributes: Attribute[];
  relationshipAttributes: RelationshipFilterAttribute[];
  pathAttributes?: { code: string; value_type: Attribute['value_type'] }[];
  attributeFilters: AttributeFilter[];
  fullHeight?: boolean;
  onBlueprintChange?: (blueprint: string) => void;
  onContextChange: (contextCode: string) => void;
  onAddAttributeFilter: (filter: AttributeFilter) => void;
  onRemoveAttributeFilter: (index: number) => void;
  onUpdateAttributeFilter: (index: number, filter: AttributeFilter) => void;
  onUpdate: (
    field: string,
    updates: { selectedIds?: string[]; targetBlueprint?: string },
  ) => void;
};

type FacetProps = Pick<Props, 'onUpdate'> & {
  facet: ExplorerRelationshipFacet;
  label: string;
  onClose: () => void;
  onOpen: () => void;
  open: boolean;
};

export const ExplorerFacetSidebar = ({
  attributeFilters,
  attributes,
  relationshipAttributes,
  blueprint,
  blueprints = [],
  contexts,
  contextCode,
  facets,
  fullHeight = false,
  onAddAttributeFilter,
  onRemoveAttributeFilter,
  onUpdateAttributeFilter,
  onBlueprintChange,
  pathAttributes,
  onContextChange,
  onUpdate,
}: Props) => {
  const { t } = useTranslation();
  const [relationshipToOpen, setRelationshipToOpen] =
    useState<RelationshipFilterAttribute>();
  const displayedFacets = relationshipToOpen
    ? [
        ...facets.filter(
          (facet) => facet.sourceRelationship.code !== relationshipToOpen.code,
        ),
        facets.find(
          (facet) => facet.sourceRelationship.code === relationshipToOpen.code,
        ) ?? {
          selectedIds: [],
          sourceRelationship: relationshipToOpen,
        },
      ]
    : facets;
  return (
    <Paper
      component="aside"
      sx={{
        alignSelf: 'start',
        borderRadius: fullHeight ? 0 : undefined,
        height: fullHeight ? '100dvh' : undefined,
        maxHeight: fullHeight ? undefined : { md: 'calc(100dvh - 104px)' },
        overflowY: fullHeight ? 'auto' : { md: 'auto' },
        p: 2,
        position: fullHeight ? 'sticky' : { md: 'sticky' },
        top: fullHeight ? 0 : { md: 88 },
      }}
    >
      {onBlueprintChange && (
        <TextField
          fullWidth
          label={t('explorer.selectBlueprint')}
          onChange={(event) => onBlueprintChange(event.target.value)}
          select
          size="small"
          value={blueprint}
        >
          {blueprints.map((item) => (
            <MenuItem key={item.code} value={item.code}>
              {item.name} ({item.code})
            </MenuItem>
          ))}
        </TextField>
      )}
      <Typography
        color="text.secondary"
        component="h2"
        sx={{
          display: 'block',
          fontWeight: 700,
          lineHeight: 1.5,
          mt: onBlueprintChange ? 2.5 : 0,
        }}
        variant="overline"
      >
        {t('explorer.filters')}
      </Typography>
      <Stack spacing={1.5} sx={{ mt: 1 }}>
        <TextField
          fullWidth
          label={t('explorer.context')}
          onChange={(event) => onContextChange(event.target.value)}
          select
          size="small"
          value={contextCode}
        >
          {contexts.map((context) => (
            <MenuItem key={context.id} value={context.code}>
              {context.code === defaultContextCode
                ? t('explorer.default')
                : context.code}
            </MenuItem>
          ))}
        </TextField>
        {displayedFacets.map((facet) => (
          <Facet
            facet={facet}
            key={facet.sourceRelationship.code}
            label={`${attributeLabel(facet.sourceRelationship)} (${blueprints.find(
              (item) =>
                item.code === facet.sourceRelationship.target_blueprint_code,
            )?.name ?? facet.sourceRelationship.target_blueprint_code})`}
            onClose={() => setRelationshipToOpen(undefined)}
            onOpen={() => setRelationshipToOpen(facet.sourceRelationship)}
            onUpdate={onUpdate}
            open={relationshipToOpen?.code === facet.sourceRelationship.code}
          />
        ))}
        <ExplorerFilterPicker
          attributes={attributes}
          blueprintName={
            blueprints.find((item) => item.code === blueprint)?.name ?? blueprint
          }
          filters={attributeFilters}
          pathAttributes={pathAttributes}
          relationshipAttributes={relationshipAttributes}
          onAdd={onAddAttributeFilter}
          onAddRelationship={setRelationshipToOpen}
          onRemove={onRemoveAttributeFilter}
          onUpdate={onUpdateAttributeFilter}
        />
      </Stack>
    </Paper>
  );
};

const Facet = ({ facet, label, onClose, onOpen, onUpdate, open }: FacetProps) => {
  const { t } = useTranslation();
  const selectionLabels = useRelationshipSelectionLabels(
    facet.sourceRelationship.target_blueprint_code,
    facet.selectedIds,
  );
  return (
    <>
      {facet.selectedIds.length > 0 && (
        <Stack spacing={0.5}>
          <Stack
            direction="row"
            spacing={0.5}
            sx={{ alignItems: 'center', color: 'primary.main' }}
          >
          <RelationshipIcon sx={{ fontSize: 15 }} />
          <Typography
            color="inherit"
            component="h3"
            sx={{ fontWeight: 700, letterSpacing: '0.01em', lineHeight: 1.4 }}
            variant="subtitle2"
          >
            {label}
          </Typography>
          </Stack>
          <RelationshipSelectionPills
            action={
              <Button
                aria-label={label}
                color="primary"
                onClick={onOpen}
                size="small"
                startIcon={<RelationshipPickerIcon fontSize="small" />}
                sx={{
                  borderRadius: 999,
                  flexShrink: 0,
                  height: 24,
                  minHeight: 24,
                  px: 1,
                  '& .MuiButton-startIcon': { mr: 0.5 },
                }}
                variant="outlined"
              >
                {t('entities.openRelationshipSelector')}
              </Button>
            }
            ids={facet.selectedIds}
            labels={selectionLabels}
            onRemove={(id) =>
              onUpdate(facet.sourceRelationship.code, {
                selectedIds: facet.selectedIds.filter(
                  (selectedId) => selectedId !== id,
                ),
              })
            }
          />
        </Stack>
      )}
      <RelationshipSelectorDialog
        closeLabel={t('entities.closeRelationshipSelector')}
        onClose={onClose}
        open={open}
        selectedLabel={t('entities.relationshipSelected', {
          count: facet.selectedIds.length,
        })}
        title={t('entities.selectRelationships', { blueprint: label })}
        topAction={{ label: t('explorer.done'), onClick: onClose }}
      >
        <RelationshipTargetPicker
          onSelectedIdsChange={(ids) =>
            onUpdate(facet.sourceRelationship.code, {
              selectedIds: ids,
              targetBlueprint:
                facet.sourceRelationship.target_blueprint_code,
            })
          }
          selectedIds={facet.selectedIds}
          targetBlueprint={facet.sourceRelationship.target_blueprint_code!}
        />
      </RelationshipSelectorDialog>
    </>
  );
};

const RelationshipTargetPicker = ({
  onSelectedIdsChange,
  selectedIds,
  targetBlueprint,
}: {
  onSelectedIdsChange: (ids: string[]) => void;
  selectedIds: string[];
  targetBlueprint: string;
}) => {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const pickerContentRoot = useRef<HTMLDivElement>(null);
  const { isPreviewed, markPreviewed, openPreview, previewHref } =
    useRecentlyPreviewedEntities((id) => {
      if (!selectedIds.includes(id))
        onSelectedIdsChange([...selectedIds, id]);
      scrollRelationshipPickerToTop(pickerContentRoot.current);
    });
  const targets = useInfiniteQuery({
    queryKey: entityQueryKeys.relationshipTargets(targetBlueprint, query),
    queryFn: ({ pageParam, signal }) =>
      searchEntities({
        blueprint: targetBlueprint,
        cursor: pageParam,
        query,
        signal,
      }),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.next_cursor,
  });
  const options = targets.data?.pages.flatMap((page) => page.items) ?? [];
  const selectedLabels = useRelationshipSelectionLabels(
    targetBlueprint,
    selectedIds,
  );
  const views = targets.data?.pages[0]?.blueprint.blueprint.views ?? {};
  const toggle = (id: string) =>
    onSelectedIdsChange(
      selectedIds.includes(id)
        ? selectedIds.filter((selectedId) => selectedId !== id)
        : [...selectedIds, id],
    );
  return (
    <Stack ref={pickerContentRoot} spacing={1.5}>
      <TextField
        fullWidth
        label={t('entities.searchRelationshipOptions')}
        onChange={(event) => setQuery(event.target.value)}
        value={query}
      />
      {selectedIds.length > 0 && (
        <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 0.5 }}>
          {selectedIds.map((id) => (
            <Chip
              color="primary"
              key={id}
              label={selectedLabels.get(id) ?? id}
              onDelete={() => toggle(id)}
              size="small"
              variant="outlined"
            />
          ))}
        </Box>
      )}
      <List dense disablePadding>
        {options.map((target) => {
          const label =
            dropdownOptionLabel(target.preview, views) ??
            displayLabel(target.display, target.id);
          return (
            <ListItem
              key={target.id}
              sx={{
                alignItems: 'center',
                bgcolor: isPreviewed(target.id) ? 'action.selected' : undefined,
                borderRadius: 1,
                '&:hover': { bgcolor: 'action.hover' },
              }}
            >
              <Link
                href={previewHref(target.id)}
                onClick={() => markPreviewed(target.id)}
                rel="opener"
                sx={{ flexGrow: 1, minWidth: 0, mr: 1 }}
                target="_blank"
              >
                {label}
              </Link>
              <Stack direction="row" spacing={0.5}>
                <Button
                  aria-label={t('entities.previewRelationshipOptionLabel', {
                    option: label,
                  })}
                  color={isPreviewed(target.id) ? 'secondary' : 'inherit'}
                  onClick={() => openPreview(target.id)}
                  size="small"
                >
                  {t('entities.previewRelationshipOption')}
                </Button>
                <Button
                  aria-label={t(
                    selectedIds.includes(target.id)
                      ? 'entities.removeRelationshipOptionLabel'
                      : 'entities.selectRelationshipOptionLabel',
                    { option: label },
                  )}
                  onClick={() => toggle(target.id)}
                  size="small"
                >
                  {selectedIds.includes(target.id)
                    ? t('entities.removeRelationshipOption')
                    : t('entities.selectRelationshipOption')}
                </Button>
              </Stack>
            </ListItem>
          );
        })}
      </List>
      {targets.hasNextPage && (
        <LoadMoreButton
          isLoading={targets.isFetchingNextPage}
          onLoadMore={() => void targets.fetchNextPage()}
        />
      )}
      {targets.isPending && (
        <Typography variant="body2">{t('entities.loadingOptions')}</Typography>
      )}
      {targets.isError && (
        <Typography color="error" variant="body2">
          {targets.error.message}
        </Typography>
      )}
    </Stack>
  );
};
