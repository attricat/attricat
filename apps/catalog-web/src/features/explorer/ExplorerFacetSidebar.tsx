import {
  Box,
  Button,
  Chip,
  MenuItem,
  List,
  ListItem,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useInfiniteQuery } from '@tanstack/react-query';
import { useState } from 'react';
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
import { ExplorerAttributeFilters } from './ExplorerAttributeFilters';
import { LoadMoreButton } from '../../components/LoadMoreButton';
import { dropdownOptionLabel, displayLabel } from '../entities/entity-display';
import type { AttributeFilter } from './search';

export type ExplorerRelationshipFacet = {
  hierarchyField?: string;
  selectedIds: string[];
  sourceRelationship: Attribute;
};

type Props = {
  blueprint: string;
  blueprints?: Blueprint[];
  contexts: AttributeContext[];
  contextCode: string;
  facets: ExplorerRelationshipFacet[];
  attributes: Attribute[];
  relationshipAttributes: Attribute[];
  pathAttributes?: { code: string; value_type: Attribute['value_type'] }[];
  attributeFilters: AttributeFilter[];
  fullHeight?: boolean;
  onBlueprintChange?: (blueprint: string) => void;
  onContextChange: (contextCode: string) => void;
  onAddAttributeFilter: (filter: AttributeFilter) => void;
  onAddRelationshipFilter: (attribute: Attribute) => void;
  onRemoveAttributeFilter: (index: number) => void;
  onUpdateAttributeFilter: (index: number, filter: AttributeFilter) => void;
  onUpdate: (
    field: string,
    updates: {
      hierarchy?: string;
      context?: string;
      selectedIds?: string[];
      targetBlueprint?: string;
    },
  ) => void;
};

type FacetProps = Pick<Props, 'onUpdate'> & {
  facet: ExplorerRelationshipFacet;
  label: string;
  onInitialPickerClose?: () => void;
  openOnMount?: boolean;
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
  onAddRelationshipFilter,
  onRemoveAttributeFilter,
  onUpdateAttributeFilter,
  onBlueprintChange,
  pathAttributes,
  onContextChange,
  onUpdate,
}: Props) => {
  const { t } = useTranslation();
  const [relationshipToOpen, setRelationshipToOpen] = useState<string>();
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
        {facets.map((facet) => (
          <Facet
            facet={facet}
            key={`${facet.sourceRelationship.code}-${relationshipToOpen === facet.sourceRelationship.code}`}
            label={
              blueprints.find(
                (item) =>
                  item.code === facet.sourceRelationship.target_blueprint_code,
              )?.name ??
              facet.sourceRelationship.target_blueprint_code ??
              facet.sourceRelationship.code
            }
            onInitialPickerClose={() => setRelationshipToOpen(undefined)}
            onUpdate={onUpdate}
            openOnMount={relationshipToOpen === facet.sourceRelationship.code}
          />
        ))}
        <ExplorerAttributeFilters
          attributes={attributes}
          blueprintName={
            blueprints.find((item) => item.code === blueprint)?.name ?? blueprint
          }
          filters={attributeFilters}
          pathAttributes={pathAttributes}
          relationshipAttributes={relationshipAttributes}
          onAdd={onAddAttributeFilter}
          onAddRelationship={(attribute) => {
            setRelationshipToOpen(attribute.code);
            onAddRelationshipFilter(attribute);
          }}
          onRemove={onRemoveAttributeFilter}
          onUpdate={onUpdateAttributeFilter}
        />
      </Stack>
    </Paper>
  );
};

const Facet = ({
  facet,
  label,
  onInitialPickerClose,
  onUpdate,
  openOnMount = false,
}: FacetProps) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(openOnMount);
  const selectionLabels = useRelationshipSelectionLabels(
    facet.sourceRelationship.target_blueprint_code,
    facet.selectedIds,
  );
  const singleSelect = false;
  const openSelector = () => {
    setOpen(true);
  };
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
                onClick={openSelector}
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
        applyLabel={t('entities.applyRelationshipSelection')}
        cancelLabel={t('entities.cancelRelationshipSelection')}
        clearLabel={t('entities.clearRelationshipSelection')}
        hideActions
        closeLabel={t('entities.closeRelationshipSelector')}
        onApply={() => {
          setOpen(false);
          onInitialPickerClose?.();
        }}
        onClear={() => undefined}
        onClose={() => {
          setOpen(false);
          onInitialPickerClose?.();
        }}
        open={open}
        selectedLabel={t('entities.relationshipSelected', {
          count: facet.selectedIds.length,
        })}
        title={t('entities.selectRelationships', { blueprint: label })}
        topActionLabel={t('explorer.done')}
      >
        <RelationshipTargetPicker
          onSelectedIdsChange={(ids) =>
            onUpdate(facet.sourceRelationship.code, { selectedIds: ids })
          }
          selectedIds={facet.selectedIds}
          singleSelect={singleSelect}
          targetBlueprint={facet.sourceRelationship.target_blueprint_code!}
        />
      </RelationshipSelectorDialog>
    </>
  );
};

const RelationshipTargetPicker = ({
  onSelectedIdsChange,
  selectedIds,
  singleSelect,
  targetBlueprint,
}: {
  onSelectedIdsChange: (ids: string[]) => void;
  selectedIds: string[];
  singleSelect: boolean;
  targetBlueprint: string;
}) => {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const targets = useInfiniteQuery({
    queryKey: entityQueryKeys.relationshipTargets(targetBlueprint, query),
    queryFn: ({ pageParam, signal }) =>
      searchEntities(targetBlueprint, undefined, query, pageParam, undefined, signal),
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
      singleSelect
        ? selectedIds[0] === id
          ? []
          : [id]
        : selectedIds.includes(id)
          ? selectedIds.filter((selectedId) => selectedId !== id)
          : [...selectedIds, id],
    );
  return (
    <Stack spacing={1.5}>
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
              secondaryAction={
                <Button onClick={() => toggle(target.id)} size="small">
                  {selectedIds.includes(target.id)
                    ? t('entities.removeRelationshipOption')
                    : t('entities.selectRelationshipOption')}
                </Button>
              }
            >
              <Typography variant="body2">{label}</Typography>
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
