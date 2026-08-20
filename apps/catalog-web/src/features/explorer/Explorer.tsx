import { useForm } from '@tanstack/react-form';
import { useQuery } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import { flexRender } from '@tanstack/react-table';
import {
  getCoreRowModel,
  legacyCreateColumnHelper,
  type LegacyColumnDef,
  useLegacyTable,
} from '@tanstack/react-table/legacy';
import {
  Alert,
  Box,
  Button,
  Chip,
  MenuItem,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import {
  listEntityBlueprints,
  getBlueprintByCode,
  listContexts,
  searchEntities,
  type EntityItem,
} from '../entities/api';
import { displayLabel } from '../entities/entity-display';
import { entityQueryKeys } from '../entities/query-keys';
import { AttributeValue } from '../views/components/values/AttributeValue';
import type { ExplorerSearch } from './search';
import { RelationshipTreeFacet } from './RelationshipTreeFacet';

export const Explorer = ({ search }: { search: ExplorerSearch }) => {
  const navigate = useNavigate({ from: '/' });
  const form = useForm({
    defaultValues: {
      blueprint: search.blueprint ?? '',
      version: search.version?.toString() ?? '',
      query: search.query ?? '',
    },
    onSubmit: ({ value }) => {
      void navigate({
        to: '/',
        search: {
          ...(value.blueprint === search.blueprint
            ? search
            : {
                facetField: undefined,
                facetHierarchy: undefined,
                facetContext: undefined,
                categories: undefined,
              }),
          blueprint: value.blueprint || undefined,
          version: value.version ? Number(value.version) : undefined,
          query: value.query || undefined,
        },
      });
    },
  });
  const contexts = useQuery({
    queryKey: entityQueryKeys.contexts(),
    queryFn: listContexts,
  });
  const selectedBlueprint = useQuery({
    queryKey: entityQueryKeys.blueprintByCode(search.blueprint, search.version),
    queryFn: () => getBlueprintByCode(search.blueprint!, search.version),
    enabled: Boolean(search.blueprint),
  });
  const sourceRelationship = selectedBlueprint.data?.attributes.find(
    (attribute) =>
      attribute.code === search.facetField &&
      attribute.value_type === 'relationship' &&
      attribute.target_blueprint_code,
  );
  const targetBlueprint = useQuery({
    queryKey: entityQueryKeys.blueprintByCode(
      sourceRelationship?.target_blueprint_code,
      undefined,
    ),
    queryFn: () =>
      getBlueprintByCode(sourceRelationship!.target_blueprint_code!),
    enabled: Boolean(sourceRelationship?.target_blueprint_code),
  });
  const hierarchyFields = (targetBlueprint.data?.attributes ?? [])
    .filter(
      (attribute) =>
        attribute.value_type === 'relationship' &&
        attribute.target_blueprint_code ===
          sourceRelationship?.target_blueprint_code,
    )
    .map((attribute) => attribute.code);
  const hierarchyField = hierarchyFields.includes(search.facetHierarchy ?? '')
    ? search.facetHierarchy
    : hierarchyFields[0];
  const contextCode = search.facetContext ?? 'default';
  const contextId = contexts.data?.find(
    (context) => context.code === contextCode,
  )?.id;
  const relationshipTreeFacet =
    sourceRelationship && hierarchyField && contextId
      ? {
          source_relationship_field: sourceRelationship.code,
          hierarchy_field: hierarchyField,
          context_id: contextId,
          selected_target_ids: search.categories ?? [],
        }
      : undefined;
  const results = useQuery({
    queryKey: entityQueryKeys.search(
      search.blueprint,
      search.version,
      search.query,
      relationshipTreeFacet,
    ),
    queryFn: () =>
      searchEntities(
        search.blueprint!,
        search.version,
        search.query ?? '',
        relationshipTreeFacet,
      ),
    enabled: Boolean(search.blueprint),
  });
  const blueprints = useQuery({
    queryKey: entityQueryKeys.blueprints(),
    queryFn: listEntityBlueprints,
  });
  const relationshipFields = (selectedBlueprint.data?.attributes ?? []).filter(
    (attribute) =>
      attribute.value_type === 'relationship' &&
      attribute.target_blueprint_code,
  );
  const updateFacet = (updates: Partial<ExplorerSearch>) => {
    void navigate({
      to: '/',
      search: { ...search, ...updates },
    });
  };

  const columnHelper = legacyCreateColumnHelper<EntityItem>();
  const tableFields =
    results.data?.blueprint.blueprint.views.table?.type === 'table'
      ? results.data.blueprint.blueprint.views.table.fields
      : [];
  const attributes = new Map(
    (results.data?.blueprint.attributes ?? []).map((attribute) => [
      attribute.code,
      attribute,
    ]),
  );
  const columns: LegacyColumnDef<EntityItem, string>[] = [
    columnHelper.accessor('id', {
      header: 'ID',
      cell: (info) => (
        <Link
          to="/entities/$entityId"
          params={{ entityId: info.row.original.id }}
        >
          {info.getValue()}
        </Link>
      ),
    }),
    columnHelper.display({
      id: 'display',
      header: 'Display',
      cell: (info) =>
        displayLabel(info.row.original.display, info.row.original.id),
    }) as LegacyColumnDef<EntityItem, string>,
    columnHelper.display({
      id: 'schema',
      header: 'Schema',
      cell: (info) => {
        const entity = info.row.original;
        return (
          <Chip
            color={entity.schema_outdated ? 'warning' : 'success'}
            label={`v${entity.blueprint_version} · ${
              entity.schema_outdated ? 'Outdated' : 'Current'
            }`}
            size="small"
          />
        );
      },
    }) as LegacyColumnDef<EntityItem, string>,
    ...tableFields.flatMap((field) => {
      const attribute = attributes.get(field);
      if (!attribute) return [];
      return [
        columnHelper.display({
          id: field,
          header: field.replaceAll('_', ' '),
          cell: (info) => (
            <AttributeValue
              attribute={attribute}
              compact
              value={info.row.original.preview.default?.[field]}
            />
          ),
        }) as LegacyColumnDef<EntityItem, string>,
      ];
    }),
  ];
  const table = useLegacyTable({
    data: results.data?.items ?? [],
    columns: columns as never,
    getCoreRowModel: getCoreRowModel(),
  });

  return (
    <PageContainer>
      <PageHeader
        description="Search a blueprint and inspect its entity projections across all schema versions."
        title="Entity explorer"
      />
      <Stack direction="row" spacing={2} sx={{ mt: 3 }}>
        <Button component={Link} to="/entities/new" variant="contained">
          Create entity
        </Button>
      </Stack>
      <Paper
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          void form.handleSubmit();
        }}
        sx={{ mt: 4, p: 2.5 }}
      >
        <Stack direction={{ xs: 'column', md: 'row' }} spacing={2}>
          <form.Field name="blueprint">
            {(field) => (
              <TextField
                required
                label="Select a Blueprint"
                onChange={(event) => {
                  field.handleChange(event.target.value);
                  form.setFieldValue('version', '');
                }}
                select
                sx={{ width: 280 }}
                value={field.state.value}
              >
                {(blueprints.data ?? []).map((blueprint) => (
                  <MenuItem key={blueprint.code} value={blueprint.code}>
                    {blueprint.name} ({blueprint.code})
                  </MenuItem>
                ))}
              </TextField>
            )}
          </form.Field>
          <form.Field name="version">
            {(field) => (
              <TextField
                inputMode="numeric"
                label="Version"
                onChange={(event) => field.handleChange(event.target.value)}
                placeholder="Current"
                value={field.state.value}
              />
            )}
          </form.Field>
          <form.Field name="query">
            {(field) => (
              <TextField
                fullWidth
                label="Query"
                onChange={(event) => field.handleChange(event.target.value)}
                placeholder="Search terms"
                value={field.state.value}
              />
            )}
          </form.Field>
          <Button type="submit" variant="contained">
            Search
          </Button>
        </Stack>
      </Paper>
      {!search.blueprint && (
        <Typography sx={{ py: 3 }}>
          Enter a blueprint code to start exploring.
        </Typography>
      )}
      {search.blueprint && results.isPending && (
        <Typography sx={{ py: 3 }}>Loading entities...</Typography>
      )}
      {results.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {results.error.message}
        </Alert>
      )}
      {results.data && (
        <Box
          sx={{
            display: 'grid',
            gap: 3,
            gridTemplateColumns: {
              xs: '1fr',
              lg: 'minmax(240px, 300px) minmax(0, 1fr)',
            },
            mt: 3,
          }}
        >
          <Paper component="aside" sx={{ alignSelf: 'start', p: 2 }}>
            <Typography component="h2" variant="subtitle1">
              Relationship facet
            </Typography>
            <TextField
              fullWidth
              label="Relationship"
              onChange={(event) =>
                updateFacet({
                  facetField: event.target.value || undefined,
                  facetHierarchy: undefined,
                  facetContext: undefined,
                  categories: undefined,
                })
              }
              select
              size="small"
              sx={{ mt: 1.5 }}
              value={search.facetField ?? ''}
            >
              <MenuItem value="">None</MenuItem>
              {relationshipFields.map((attribute) => (
                <MenuItem key={attribute.code} value={attribute.code}>
                  {attribute.code}
                </MenuItem>
              ))}
            </TextField>
            {search.facetField && targetBlueprint.isPending && (
              <Typography color="text.secondary" sx={{ mt: 2 }} variant="body2">
                Loading category tree...
              </Typography>
            )}
            {search.facetField &&
              !targetBlueprint.isPending &&
              !hierarchyFields.length && (
                <Typography
                  color="text.secondary"
                  sx={{ mt: 2 }}
                  variant="body2"
                >
                  This relationship target has no self-referencing relationship.
                </Typography>
              )}
            {hierarchyField && (
              <RelationshipTreeFacet
                contextCode={contextCode}
                contexts={contexts.data ?? []}
                hierarchyField={hierarchyField}
                hierarchyFields={hierarchyFields}
                items={results.data.relationship_tree_facet?.items}
                onContextChange={(facetContext) =>
                  updateFacet({ facetContext, categories: undefined })
                }
                onHierarchyFieldChange={(facetHierarchy) =>
                  updateFacet({ facetHierarchy, categories: undefined })
                }
                onSelectedIdsChange={(categories) =>
                  updateFacet({ categories })
                }
                selectedIds={search.categories ?? []}
              />
            )}
          </Paper>
          <Paper component="section">
            <Box sx={{ borderBottom: 1, borderColor: 'divider', p: 2 }}>
              <Typography>
                <strong>{results.data.blueprint.blueprint.code}</strong> v
                {results.data.blueprint.blueprint.version} ·{' '}
                {results.data.items.length} result
                {results.data.items.length === 1 ? '' : 's'}
              </Typography>
            </Box>
            <TableContainer>
              <Table size="small">
                <TableHead>
                  {table.getHeaderGroups().map((group) => (
                    <TableRow key={group.id}>
                      {group.headers.map((header) => (
                        <TableCell key={header.id}>
                          {header.isPlaceholder
                            ? null
                            : flexRender(
                                header.column.columnDef.header,
                                header.getContext(),
                              )}
                        </TableCell>
                      ))}
                    </TableRow>
                  ))}
                </TableHead>
                <TableBody>
                  {table.getRowModel().rows.map((row) => (
                    <TableRow key={row.id}>
                      {row.getVisibleCells().map((cell) => (
                        <TableCell key={cell.id}>
                          {flexRender(
                            cell.column.columnDef.cell,
                            cell.getContext(),
                          )}
                        </TableCell>
                      ))}
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </TableContainer>
            {results.data.items.length === 0 && (
              <Typography sx={{ p: 2 }}>
                No entities matched this search.
              </Typography>
            )}
            {results.data.next_cursor && (
              <Alert severity="info" sx={{ m: 2 }}>
                More results are available. Pagination will be added with the
                search cursor.
              </Alert>
            )}
          </Paper>
        </Box>
      )}
    </PageContainer>
  );
};
