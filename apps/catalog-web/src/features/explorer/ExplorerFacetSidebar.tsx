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
import type { Attribute } from '../entities/api';
import type { AttributeContext } from '../contexts/api';
import { getBlueprintByCode } from '../entities/api';
import { entityQueryKeys } from '../entities/query-keys';
import { RelationshipTreeFacet } from './RelationshipTreeFacet';

export type ExplorerRelationshipFacet = {
  hierarchyField?: string;
  selectedIds: string[];
  sourceRelationship: Attribute;
};

type Props = {
  blueprint: string;
  contexts: AttributeContext[];
  contextCode: string;
  facets: ExplorerRelationshipFacet[];
  query?: string;
  version?: number;
  onContextChange: (contextCode: string) => void;
  onUpdate: (
    field: string,
    updates: {
      hierarchy?: string;
      context?: string;
      selectedIds?: string[];
    },
  ) => void;
};

type FacetProps = Omit<Props, 'facets' | 'onContextChange'> & {
  facet: ExplorerRelationshipFacet;
};

export const ExplorerFacetSidebar = ({
  blueprint,
  contexts,
  contextCode,
  facets,
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
        maxHeight: { md: 'calc(100dvh - 104px)' },
        overflowY: { md: 'auto' },
        p: 2,
        position: { md: 'sticky' },
        top: { md: 88 },
      }}
    >
      <Typography variant="subtitle2">
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
