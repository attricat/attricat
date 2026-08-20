import { Link } from '@tanstack/react-router';
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
  Chip,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import type { EntitySearchResponse, EntityItem } from '../entities/api';
import { displayLabel } from '../entities/entity-display';
import { AttributeValue } from '../views/components/values/AttributeValue';

export const ExplorerResultsTable = ({
  results,
}: {
  results: EntitySearchResponse;
}) => {
  const columnHelper = legacyCreateColumnHelper<EntityItem>();
  const tableFields =
    results.blueprint.blueprint.views.table?.type === 'table'
      ? results.blueprint.blueprint.views.table.fields
      : [];
  const attributes = new Map(
    results.blueprint.attributes.map((attribute) => [attribute.code, attribute]),
  );
  const columns: LegacyColumnDef<EntityItem, string>[] = [
    columnHelper.accessor('id', {
      header: 'ID',
      cell: (info) => (
        <Link to="/entities/$entityId" params={{ entityId: info.row.original.id }}>
          {info.getValue()}
        </Link>
      ),
    }),
    columnHelper.display({
      id: 'display',
      header: 'Display',
      cell: (info) => displayLabel(info.row.original.display, info.row.original.id),
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
    data: results.items,
    columns: columns as never,
    getCoreRowModel: getCoreRowModel(),
  });

  return (
    <Paper component="section">
      <Box sx={{ borderBottom: 1, borderColor: 'divider', p: 2 }}>
        <Typography>
          <strong>{results.blueprint.blueprint.code}</strong> v
          {results.blueprint.blueprint.version} · {results.items.length} result
          {results.items.length === 1 ? '' : 's'}
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
                      : flexRender(header.column.columnDef.header, header.getContext())}
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
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </TableCell>
                ))}
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </TableContainer>
      {results.items.length === 0 && (
        <Typography sx={{ p: 2 }}>No entities matched this search.</Typography>
      )}
      {results.next_cursor && (
        <Alert severity="info" sx={{ m: 2 }}>
          More results are available. Pagination will be added with the search
          cursor.
        </Alert>
      )}
    </Paper>
  );
};
