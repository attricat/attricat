import { useMutation, useQuery } from '@tanstack/react-query'
import { useForm } from '@tanstack/react-form'
import {
  Link,
  Outlet,
  createRootRoute,
  createRoute,
  createRouter,
  useNavigate,
} from '@tanstack/react-router'
import { flexRender } from '@tanstack/react-table'
import {
  getCoreRowModel,
  legacyCreateColumnHelper,
  type LegacyColumnDef,
  useLegacyTable,
} from '@tanstack/react-table/legacy'
import {
  Alert,
  Box,
  Button,
  Container,
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
} from '@mui/material'
import {
  createEntity,
  getBlueprintByCode,
  listEntityBlueprints,
  getEntityForm,
  getEntityPreview,
  searchEntities,
  updateEntity,
  type EntityItem,
} from './api'
import { EntityForm } from './EntityForm'
import { valuesForForm } from './entity-form'
import { displayLabel, parseExplorerSearch } from './search'

const rootRoute = createRootRoute({ component: () => <Outlet /> })

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  validateSearch: parseExplorerSearch,
  component: Explorer,
})

const previewRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/entities/$entityId',
  component: EntityPreview,
})

const newEntityRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/entities/new',
  component: NewEntity,
})

const editEntityRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/entities/$entityId/edit',
  component: EditEntity,
})

const routeTree = rootRoute.addChildren([
  indexRoute,
  newEntityRoute,
  editEntityRoute,
  previewRoute,
])
export const router = createRouter({ routeTree })

function Explorer() {
  const search = indexRoute.useSearch()
  const navigate = useNavigate({ from: indexRoute.fullPath })
  const form = useForm({
    defaultValues: {
      blueprint: search.blueprint ?? '',
      version: search.version?.toString() ?? '',
      query: search.query ?? '',
    },
    onSubmit: ({ value }) => {
      void navigate({ to: '/', search: parseExplorerSearch(value) })
    },
  })
  const results = useQuery({
    queryKey: ['entities', search.blueprint, search.version, search.query],
    queryFn: () =>
      searchEntities(search.blueprint!, search.version, search.query ?? ''),
    enabled: Boolean(search.blueprint),
  })
  const blueprints = useQuery({
    queryKey: ['entity-blueprints'],
    queryFn: listEntityBlueprints,
  })

  const columnHelper = legacyCreateColumnHelper<EntityItem>()
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
  ]
  const table = useLegacyTable({
    data: results.data?.items ?? [],
    columns: columns as never,
    getCoreRowModel: getCoreRowModel(),
  })

  return (
    <Container component="main" maxWidth="xl" sx={{ py: { xs: 4, md: 7 } }}>
      <Typography
        color="primary"
        sx={{
          fontWeight: 700,
          letterSpacing: '.12em',
          textTransform: 'uppercase',
        }}
        variant="overline"
      >
        Catalog
      </Typography>
      <Typography component="h1" variant="h2">
        Entity explorer
      </Typography>
      <Typography color="text.secondary">
        Search a blueprint and inspect its current entity projections.
      </Typography>
      <Button
        component={Link}
        to="/entities/new"
        sx={{ mt: 2 }}
        variant="contained"
      >
        Create entity
      </Button>
      <Paper
        component="form"
        onSubmit={(event) => {
          event.preventDefault()
          void form.handleSubmit()
        }}
        sx={{ mt: 4, p: 2.5 }}
      >
        <Stack direction={{ xs: 'column', md: 'row' }} spacing={2}>
          <form.Field name="blueprint">
            {(field) => (
              <TextField
                required
                label="Blueprint"
                onChange={(event) => {
                  field.handleChange(event.target.value)
                  form.setFieldValue('version', '')
                }}
                select
                slotProps={{ select: { displayEmpty: true } }}
                sx={{ width: 280 }}
                value={field.state.value}
              >
                <MenuItem disabled value="">
                  Select a blueprint
                </MenuItem>
                {blueprints.data?.map((blueprint) => (
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
      {results.isPending && (
        <Typography sx={{ py: 3 }}>Loading entities...</Typography>
      )}
      {results.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {results.error.message}
        </Alert>
      )}
      {results.data && (
        <Paper component="section" sx={{ mt: 3 }}>
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
      )}
    </Container>
  )
}

function EntityPreview() {
  const { entityId } = previewRoute.useParams()
  const preview = useQuery({
    queryKey: ['entity-preview', entityId],
    queryFn: () => getEntityPreview(entityId),
  })
  return (
    <Container component="main" maxWidth="lg" sx={{ py: { xs: 4, md: 7 } }}>
      <Button component={Link} to="/" sx={{ mb: 4 }}>
        Back to explorer
      </Button>
      <Typography
        color="primary"
        sx={{
          fontWeight: 700,
          letterSpacing: '.12em',
          textTransform: 'uppercase',
        }}
        variant="overline"
      >
        Entity preview
      </Typography>
      <Typography component="h1" variant="h3">
        {entityId}
      </Typography>
      <Box sx={{ mt: 1 }}>
        <Link params={{ entityId }} to="/entities/$entityId/edit">
          Edit entity
        </Link>
      </Box>
      {preview.isPending && (
        <Typography sx={{ py: 3 }}>Loading preview...</Typography>
      )}
      {preview.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {preview.error.message}
        </Alert>
      )}
      {preview.data && (
        <Paper component="pre" sx={{ mt: 3, overflow: 'auto', p: 3 }}>
          {JSON.stringify(preview.data, null, 2)}
        </Paper>
      )}
    </Container>
  )
}

function NewEntity() {
  const navigate = useNavigate({ from: newEntityRoute.fullPath })
  const blueprint = useMutation({
    mutationFn: ({ code, version }: { code: string; version?: number }) =>
      getBlueprintByCode(code, version),
  })
  const create = useMutation({
    mutationFn: ({
      values,
      relationships,
    }: {
      values: Parameters<typeof createEntity>[0]['values']
      relationships: { attribute_code: string; target_entity_ids: string[] }[]
    }) => {
      const resolved = blueprint.data
      if (!resolved)
        throw new Error('Choose a blueprint before creating an entity')
      return createEntity({
        blueprint: {
          code: resolved.blueprint.code,
          version: resolved.blueprint.version,
        },
        values: [
          ...values,
          ...relationships.flatMap((relationship) =>
            relationship.target_entity_ids.map((target_entity_id) => ({
              kind: 'relationship' as const,
              attribute_code: relationship.attribute_code,
              target_entity_id,
            })),
          ),
        ],
      })
    },
    onSuccess: (entity) => {
      void navigate({
        to: '/entities/$entityId',
        params: { entityId: entity.id },
      })
    },
  })
  return (
    <EntityPage title="Create entity">
      <EntityForm
        blueprint={blueprint.data}
        error={blueprint.error ?? create.error}
        isLoadingBlueprint={blueprint.isPending || create.isPending}
        onLoadBlueprint={(code, version) => blueprint.mutate({ code, version })}
        onSubmit={(input) => create.mutate(input)}
        submitLabel={blueprint.data ? 'Create entity' : 'Load blueprint'}
      />
    </EntityPage>
  )
}

function EditEntity() {
  const { entityId } = editEntityRoute.useParams()
  const navigate = useNavigate({ from: editEntityRoute.fullPath })
  const entityForm = useQuery({
    queryKey: ['entity-form', entityId],
    queryFn: () => getEntityForm(entityId),
  })
  const update = useMutation({
    mutationFn: (input: Parameters<typeof updateEntity>[1]) =>
      updateEntity(entityId, input),
    onSuccess: (entity) => {
      void navigate({
        to: '/entities/$entityId',
        params: { entityId: entity.id },
      })
    },
  })
  return (
    <EntityPage title="Edit entity">
      {entityForm.isPending && (
        <Typography sx={{ mt: 4 }}>Loading entity...</Typography>
      )}
      {(entityForm.error || update.error) && (
        <Alert severity="error" sx={{ mt: 4 }}>
          {(entityForm.error ?? update.error)?.message}
        </Alert>
      )}
      {entityForm.data && (
        <EntityForm
          key={entityForm.data.entity.id}
          blueprint={entityForm.data.blueprint}
          initialValues={valuesForForm(
            entityForm.data.blueprint.attributes,
            entityForm.data.values,
          )}
          isLoadingBlueprint={update.isPending}
          onSubmit={(input) => update.mutate(input)}
          submitLabel="Save changes"
        />
      )}
    </EntityPage>
  )
}

function EntityPage({
  children,
  title,
}: {
  children: import('react').ReactNode
  title: string
}) {
  return (
    <Container component="main" maxWidth="md" sx={{ py: { xs: 4, md: 7 } }}>
      <Button component={Link} to="/" sx={{ mb: 4 }}>
        Back to explorer
      </Button>
      <Typography
        color="primary"
        sx={{
          fontWeight: 700,
          letterSpacing: '.12em',
          textTransform: 'uppercase',
        }}
        variant="overline"
      >
        Catalog
      </Typography>
      <Typography component="h1" variant="h3">
        {title}
      </Typography>
      {children}
    </Container>
  )
}
