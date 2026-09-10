import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
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

type FacetProps = Omit<
  Props,
  | 'activeAttributeFilterCount'
  | 'attributes'
  | 'facets'
  | 'onAddAttributeFilter'
  | 'onContextChange'
> & {
  facet: ExplorerRelationshipFacet;
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
  onUpdate,
  query,
  version,
}: FacetProps) => {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(facet.selectedIds.length > 0);
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
    enabled: expanded,
  });
  const hierarchyFields = (targetBlueprint.data?.attributes ?? [])
    .filter(
      (attribute) =>
        attribute.value_type === 'relationship' &&
        attribute.target_blueprint_code ===
          facet.sourceRelationship.target_blueprint_code,
    )
    .map((attribute) => attribute.code);
  const hierarchyField = hierarchyFields.includes(facet.hierarchyField ?? '')
    ? facet.hierarchyField
    : hierarchyFields[0];

  return (
    <Accordion
      disableGutters
      elevation={0}
      expanded={expanded}
      onChange={(_, isExpanded) => setExpanded(isExpanded)}
      sx={{ '&:before': { display: 'none' }, mt: 1 }}
    >
      <AccordionSummary expandIcon={<ExpandMoreIcon />}>
        <Typography variant="body2">{facet.sourceRelationship.code}</Typography>
      </AccordionSummary>
      <AccordionDetails>
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
            onHierarchyFieldChange={(hierarchy) =>
              onUpdate(facet.sourceRelationship.code, {
                hierarchy,
                selectedIds: [],
              })
            }
            onSelectedIdsChange={(selectedIds) =>
              onUpdate(facet.sourceRelationship.code, {
                ...(hierarchyField ? { hierarchy: hierarchyField } : {}),
                selectedIds,
              })
            }
            query={query}
            selectedIds={facet.selectedIds}
            sourceField={facet.sourceRelationship.code}
            version={version}
          />
        )}
      </AccordionDetails>
    </Accordion>
  );
};
