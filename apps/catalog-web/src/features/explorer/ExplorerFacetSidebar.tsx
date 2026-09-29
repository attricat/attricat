import { MenuItem, Paper, Stack, TextField, Typography } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { AttributeContext } from '../contexts/api';
import { defaultContextCode } from '../contexts/constants';
import type { Attribute, Blueprint } from '../entities/api';
import { attributeLabel } from '../entities/entityDisplay';
import { facetSidebarMaxHeight, facetSidebarStickyTop } from './constants';
import { ExplorerFilterPicker } from './ExplorerFilterPicker';
import { RelationshipFacetFilter } from './RelationshipFacetFilter';
import type {
  ExplorerRelationshipFacet,
  RelationshipFacetUpdate,
  RelationshipFilterAttribute,
} from './relationshipFilterTypes';
import type { AttributeFilterRequest } from './attributeFilterValues';
import type { AttributeFilter } from './search';

type Props = {
  blueprint: string;
  filterRequest?: AttributeFilterRequest;
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
  onUpdate: (field: string, updates: RelationshipFacetUpdate) => void;
};

/** Keeps the facet being picked visible even before it has selections. */
const withOpenFacet = (
  facets: ExplorerRelationshipFacet[],
  relationshipToOpen: RelationshipFilterAttribute | undefined,
) =>
  relationshipToOpen
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

export const ExplorerFacetSidebar = ({
  attributeFilters,
  attributes,
  relationshipAttributes,
  blueprint,
  blueprints = [],
  contexts,
  contextCode,
  facets,
  filterRequest,
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
  const blueprintName = (code: string) =>
    blueprints.find((item) => item.code === code)?.name ?? code;

  return (
    <Paper
      component="aside"
      sx={{
        alignSelf: 'start',
        borderRadius: fullHeight ? 0 : undefined,
        height: fullHeight ? '100dvh' : undefined,
        maxHeight: fullHeight ? undefined : { md: facetSidebarMaxHeight },
        overflowY: fullHeight ? 'auto' : { md: 'auto' },
        px: 4,
        py: fullHeight ? 8 : 4,
        position: fullHeight ? 'sticky' : { md: 'sticky' },
        top: fullHeight ? 0 : { md: facetSidebarStickyTop },
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
              {t('explorer.blueprintOption', {
                code: item.code,
                name: item.name,
              })}
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
      <Stack spacing={1.5} sx={{ mt: 2 }}>
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
        {withOpenFacet(facets, relationshipToOpen).map((facet) => (
          <RelationshipFacetFilter
            facet={facet}
            key={facet.sourceRelationship.code}
            label={t('explorer.relationshipFacetLabel', {
              attribute: attributeLabel(facet.sourceRelationship),
              blueprint: blueprintName(
                facet.sourceRelationship.target_blueprint_code,
              ),
            })}
            onClose={() => setRelationshipToOpen(undefined)}
            onOpen={() => setRelationshipToOpen(facet.sourceRelationship)}
            onUpdate={onUpdate}
            open={relationshipToOpen?.code === facet.sourceRelationship.code}
          />
        ))}
        <ExplorerFilterPicker
          attributes={attributes}
          blueprintName={blueprintName(blueprint)}
          filterRequest={filterRequest}
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
