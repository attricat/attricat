import { MenuItem, Paper, TextField, Typography } from '@mui/material';
import type { Attribute, AttributeContext } from '../entities/api';
import { RelationshipTreeFacet } from './RelationshipTreeFacet';

type Props = {
  blueprint: string;
  contextCode: string;
  contexts: AttributeContext[];
  hierarchyField?: string;
  hierarchyFields: string[];
  isTargetBlueprintPending: boolean;
  query?: string;
  relationshipFields: Attribute[];
  searchFacetField?: string;
  selectedIds: string[];
  sourceRelationship?: Attribute;
  version?: number;
  onUpdate: (updates: {
    facetField?: string;
    facetHierarchy?: string;
    facetContext?: string;
    categories?: string[];
  }) => void;
};

export const ExplorerFacetSidebar = ({
  blueprint,
  contextCode,
  contexts,
  hierarchyField,
  hierarchyFields,
  isTargetBlueprintPending,
  query,
  relationshipFields,
  searchFacetField,
  selectedIds,
  sourceRelationship,
  version,
  onUpdate,
}: Props) => (
  <Paper component="aside" sx={{ alignSelf: 'start', p: 2 }}>
    <TextField
      fullWidth
      label="Relationship"
      onChange={(event) =>
        onUpdate({
          facetField: event.target.value || undefined,
          facetHierarchy: undefined,
          facetContext: undefined,
          categories: undefined,
        })
      }
      select
      size="small"
      sx={{ mt: 1.5 }}
      value={searchFacetField ?? ''}
    >
      <MenuItem value="">None</MenuItem>
      {relationshipFields.map((attribute) => (
        <MenuItem key={attribute.code} value={attribute.code}>
          {attribute.code}
        </MenuItem>
      ))}
    </TextField>
    {searchFacetField && isTargetBlueprintPending && (
      <Typography color="text.secondary" sx={{ mt: 2 }} variant="body2">
        Loading category tree...
      </Typography>
    )}
    {searchFacetField && !isTargetBlueprintPending && !hierarchyFields.length && (
      <Typography color="text.secondary" sx={{ mt: 2 }} variant="body2">
        This relationship target has no self-referencing relationship.
      </Typography>
    )}
    {hierarchyField && sourceRelationship && (
      <RelationshipTreeFacet
        blueprint={blueprint}
        contextCode={contextCode}
        contexts={contexts}
        hierarchyField={hierarchyField}
        hierarchyFields={hierarchyFields}
        onContextChange={(facetContext) =>
          onUpdate({ facetContext, categories: undefined })
        }
        onHierarchyFieldChange={(facetHierarchy) =>
          onUpdate({ facetHierarchy, categories: undefined })
        }
        onSelectedIdsChange={(categories) => onUpdate({ categories })}
        selectedIds={selectedIds}
        query={query}
        sourceField={sourceRelationship.code}
        version={version}
      />
    )}
  </Paper>
);
