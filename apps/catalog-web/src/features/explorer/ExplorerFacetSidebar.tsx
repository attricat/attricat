import { Paper, Typography } from '@mui/material';
import type {
  Attribute,
  AttributeContext,
  BlueprintWithAttributes,
} from '../entities/api';
import { RelationshipTreeFacet } from './RelationshipTreeFacet';

export type ExplorerRelationshipFacet = {
  contextCode: string;
  hierarchyField?: string;
  hierarchyFields: string[];
  selectedIds: string[];
  sourceRelationship: Attribute;
  targetBlueprint?: BlueprintWithAttributes;
};

type Props = {
  blueprint: string;
  contexts: AttributeContext[];
  facets: ExplorerRelationshipFacet[];
  query?: string;
  version?: number;
  onUpdate: (
    field: string,
    updates: {
      hierarchy?: string;
      context?: string;
      selectedIds?: string[];
    },
  ) => void;
};

export const ExplorerFacetSidebar = ({
  blueprint,
  contexts,
  facets,
  query,
  version,
  onUpdate,
}: Props) => (
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
    <Typography variant="subtitle2">Relationship filters</Typography>
    {facets.map((facet) => (
      <section key={facet.sourceRelationship.code}>
        <Typography sx={{ mt: 2 }} variant="body2">
          {facet.sourceRelationship.code}
        </Typography>
        {!facet.targetBlueprint ? (
          <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
            Loading facet...
          </Typography>
        ) : (
          <RelationshipTreeFacet
            blueprint={blueprint}
            contextCode={facet.contextCode}
            contexts={contexts}
            hierarchyField={facet.hierarchyField}
            hierarchyFields={facet.hierarchyFields}
            onContextChange={(context) =>
              onUpdate(facet.sourceRelationship.code, {
                context,
                selectedIds: [],
              })
            }
            onHierarchyFieldChange={(hierarchy) =>
              onUpdate(facet.sourceRelationship.code, {
                hierarchy,
                selectedIds: [],
              })
            }
            onSelectedIdsChange={(selectedIds) =>
              onUpdate(facet.sourceRelationship.code, { selectedIds })
            }
            query={query}
            selectedIds={facet.selectedIds}
            sourceField={facet.sourceRelationship.code}
            version={version}
          />
        )}
      </section>
    ))}
  </Paper>
);
