import { useForm } from '@tanstack/react-form';
import { useQuery } from '@tanstack/react-query';
import { Link, createFileRoute, useNavigate } from '@tanstack/react-router';
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
} from '@mui/material';
import { listEntityBlueprints, searchEntities, type EntityItem } from '../api';
import { displayLabel, parseExplorerSearch } from '../search';

export const Route = createFileRoute('/')({
  validateSearch: parseExplorerSearch,
  component: () => <Explorer />,
});

const Explorer = () => {
  const search = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  const form = useForm({
    defaultValues: {
      blueprint: search.blueprint ?? '',
      version: search.version?.toString() ?? '',
      query: search.query ?? '',
    },
    onSubmit: ({ value }) => {
      void navigate({ to: '/', search: parseExplorerSearch(value) });
    },
  });
  const results = useQuery({
    queryKey: ['entities', search.blueprint, search.version, search.query],
    queryFn: () =>
      searchEntities(search.blueprint!, search.version, search.query ?? ''),
    enabled: Boolean(search.blueprint),
  });
  const blueprints = useQuery({
    queryKey: ['entity-blueprints'],
    queryFn: listEntityBlueprints,
  });

  const columnHelper = legacyCreateColumnHelper<EntityItem>();
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
  ];
  const table = useLegacyTable({
    data: results.data?.items ?? [],
    columns: columns as never,
    getCoreRowModel: getCoreRowModel(),
  });

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
  );
};
