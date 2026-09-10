import {
  Button,
  CircularProgress,
  MenuItem,
  Paper,
  TextField,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { Attribute, Blueprint } from '../entities/api';
import type { AttributeContext } from '../contexts/api';
import { getBlueprintByCode } from '../entities/api';
import { entityQueryKeys } from '../entities/query-keys';
import { RelationshipSelectorDialog } from '../../components/RelationshipSelectorDialog';
import { RelationshipIcon } from '../../components/system-icons';
import { ExplorerAttributeFilters } from './ExplorerAttributeFilters';
import { RelationshipTreeFacet } from './RelationshipTreeFacet';
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
  activeAttributeFilterCount: number;
  query?: string;
  version?: number;
  fullHeight?: boolean;
  onBlueprintChange?: (blueprint: string) => void;
  onContextChange: (contextCode: string) => void;
  onAddAttributeFilter: (filter: AttributeFilter) => void;
  onUpdate: (
    field: string,
    updates: {
      hierarchy?: string;
      context?: string;
      selectedIds?: string[];
    },
  ) => void;
};

type FacetProps = Pick<
  Props,
  'blueprint' | 'contextCode' | 'contexts' | 'onUpdate' | 'query' | 'version'
> & {
  facet: ExplorerRelationshipFacet;
  label: string;
};

export const ExplorerFacetSidebar = ({
  activeAttributeFilterCount,
  attributes,
  blueprint,
  blueprints = [],
  contexts,
  contextCode,
  facets,
  fullHeight = false,
  onAddAttributeFilter,
  onBlueprintChange,
  query,
  version,
  onContextChange,
  onUpdate,
}: Props) => {
  const { t } = useTranslation();
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
      <Typography sx={{ mt: onBlueprintChange ? 2 : 0 }} variant="subtitle2">
        {t('explorer.relationshipFilters')}
      </Typography>
      <TextField
        fullWidth
        label={t('explorer.context')}
        onChange={(event) => onContextChange(event.target.value)}
        select
        size="small"
        sx={{ mt: 1 }}
        value={contextCode}
      >
        {contexts.map((context) => (
          <MenuItem key={context.id} value={context.code}>
            {context.code === 'default' ? t('explorer.default') : context.code}
          </MenuItem>
        ))}
      </TextField>
      {facets.map((facet) => (
        <Facet
          blueprint={blueprint}
          contextCode={contextCode}
          contexts={contexts}
          facet={facet}
          key={facet.sourceRelationship.code}
          label={
            blueprints.find(
              (item) =>
                item.code === facet.sourceRelationship.target_blueprint_code,
            )?.name ??
            facet.sourceRelationship.target_blueprint_code ??
            facet.sourceRelationship.code
          }
          onUpdate={onUpdate}
          query={query}
          version={version}
        />
      ))}
      <Typography sx={{ mt: 2 }} variant="subtitle2">
        {t('explorer.attributeFilters')}
      </Typography>
      <ExplorerAttributeFilters
        activeFilterCount={activeAttributeFilterCount}
        attributes={attributes}
        onAdd={onAddAttributeFilter}
      />
    </Paper>
  );
};

const Facet = ({
  blueprint,
  contextCode,
  contexts,
  facet,
  label,
  onUpdate,
  query,
  version,
}: FacetProps) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [draftIds, setDraftIds] = useState<string[]>([]);
  const [draftHierarchy, setDraftHierarchy] = useState(
    facet.hierarchyField ?? '',
  );
  const targetBlueprint = useQuery({
    queryKey: entityQueryKeys.blueprintByCode(
      facet.sourceRelationship.target_blueprint_code ?? undefined,
      undefined,
    ),
    queryFn: ({ signal }) =>
      getBlueprintByCode(
        facet.sourceRelationship.target_blueprint_code!,
        undefined,
        signal,
      ),
    enabled: open,
  });
  const hierarchyFields = (targetBlueprint.data?.attributes ?? [])
    .filter(
      (attribute) =>
        attribute.value_type === 'relationship' &&
        attribute.target_blueprint_code ===
          facet.sourceRelationship.target_blueprint_code,
    )
    .map((attribute) => attribute.code);
  const hierarchyField = hierarchyFields.includes(draftHierarchy)
    ? draftHierarchy
    : hierarchyFields[0];

  const singleSelect =
    facet.sourceRelationship.relationship_cardinality === 'one_to_one';
  const openSelector = () => {
    setDraftIds(
      singleSelect ? facet.selectedIds.slice(0, 1) : facet.selectedIds,
    );
    setDraftHierarchy(facet.hierarchyField ?? '');
    setOpen(true);
  };

  return (
    <>
      <Button
        aria-label={label}
        fullWidth
        onClick={openSelector}
        startIcon={<RelationshipIcon />}
        sx={{ justifyContent: 'space-between', mt: 1 }}
        variant="outlined"
      >
        <span>{label}</span>
        <Typography color="text.secondary" component="span" variant="body2">
          {t('entities.relationshipSelected', {
            count: facet.selectedIds.length,
          })}
        </Typography>
      </Button>
      <RelationshipSelectorDialog
        applyLabel={t('entities.applyRelationshipSelection')}
        cancelLabel={t('entities.cancelRelationshipSelection')}
        clearLabel={t('entities.clearRelationshipSelection')}
        closeLabel={t('entities.closeRelationshipSelector')}
        onApply={() => {
          onUpdate(facet.sourceRelationship.code, {
            ...(hierarchyField ? { hierarchy: hierarchyField } : {}),
            selectedIds: singleSelect ? draftIds.slice(0, 1) : draftIds,
          });
          setOpen(false);
        }}
        onClear={() => setDraftIds([])}
        onClose={() => setOpen(false)}
        open={open}
        selectedLabel={t('entities.relationshipSelected', {
          count: draftIds.length,
        })}
        title={t(
          singleSelect
            ? 'entities.selectOneRelationship'
            : 'entities.selectRelationships',
          { blueprint: label },
        )}
      >
        {targetBlueprint.isPending ? (
          <CircularProgress
            aria-label={t('explorer.loadingFacetOptions')}
            size={20}
          />
        ) : targetBlueprint.isError ? (
          <Typography color="error" variant="body2">
            {targetBlueprint.error.message}
          </Typography>
        ) : (
          <RelationshipTreeFacet
            blueprint={blueprint}
            contextCode={contextCode}
            contexts={contexts}
            hierarchyField={hierarchyField}
            hierarchyFields={hierarchyFields}
            onHierarchyFieldChange={(hierarchy) => {
              setDraftHierarchy(hierarchy);
              setDraftIds([]);
            }}
            onSelectedIdsChange={setDraftIds}
            query={query}
            selectedIds={draftIds}
            singleSelect={singleSelect}
            sourceField={facet.sourceRelationship.code}
            version={version}
          />
        )}
      </RelationshipSelectorDialog>
    </>
  );
};
