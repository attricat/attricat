import { Link } from '@tanstack/react-router';
import { flexRender } from '@tanstack/react-table';
import { useVirtualizer } from '@tanstack/react-virtual';
import {
  getCoreRowModel,
  legacyCreateColumnHelper,
  type LegacyColumnDef,
  useLegacyTable,
} from '@tanstack/react-table/legacy';
import {
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
import { LoadMoreButton } from '../../components/LoadMoreButton';
import { useRef } from 'react';
import type { BlueprintWithAttributes, EntityItem } from '../entities/api';
import { displayLabel } from '../entities/entity-display';
import { AttributeValue } from '../views/components/values/AttributeValue';

export const ExplorerResultsTable = ({
  blueprint,
  hasNextPage,
  isFetchingNextPage,
  items,
  onLoadMore,
}: {
  blueprint: BlueprintWithAttributes;
  hasNextPage: boolean;
  isFetchingNextPage: boolean;
  items: EntityItem[];
  onLoadMore: () => void;
}) => {
  const columnHelper = legacyCreateColumnHelper<EntityItem>();
  const tableFields =
    blueprint.blueprint.views.table?.type === 'table'
      ? blueprint.blueprint.views.table.fields
      : [];
  const attributes = new Map(
    blueprint.attributes.map((attribute) => [attribute.code, attribute]),
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
    data: items,
    columns: columns as never,
    getCoreRowModel: getCoreRowModel(),
  });
  const rows = table.getRowModel().rows;
  const tableContainerRef = useRef<HTMLDivElement>(null);
  // TanStack Virtual owns imperative scroll measurements and is intentionally
  // excluded from React Compiler memoization.
  // eslint-disable-next-line react-hooks/incompatible-library
  const rowVirtualizer = useVirtualizer({
    count: rows.length + (hasNextPage ? 1 : 0),
    estimateSize: () => 53,
    getScrollElement: () => tableContainerRef.current,
    measureElement: (element) => element?.getBoundingClientRect().height,
    overscan: 10,
  });
  const virtualRows = rowVirtualizer.getVirtualItems();
  const paddingTop = virtualRows[0]?.start ?? 0;
  const paddingBottom =
    rowVirtualizer.getTotalSize() - (virtualRows.at(-1)?.end ?? 0);

  return (
    <Paper component="section">
      <Box sx={{ borderBottom: 1, borderColor: 'divider', p: 2 }}>
        <Typography>
          <strong>{blueprint.blueprint.code}</strong> v
          {blueprint.blueprint.version} · {items.length} result
          {items.length === 1 ? '' : 's'}
        </Typography>
      </Box>
      <TableContainer
        aria-label="Explorer results"
        ref={tableContainerRef}
        sx={{
          // On desktop this leaves room for the sticky search form and result
          // summary while using the rest of the viewport for rows.
          height: {
            xs: 'calc(100dvh - 220px)',
            md: 'calc(100dvh - 165px)',
          },
          overflowY: 'auto',
        }}
      >
        <Table size="small" stickyHeader>
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
            {paddingTop > 0 && (
              <TableRow>
                <TableCell
                  colSpan={columns.length}
                  sx={{ height: paddingTop, p: 0 }}
                />
              </TableRow>
            )}
            {virtualRows.map((virtualRow) => {
              if (virtualRow.index === rows.length) {
                return (
                  <TableRow data-index={virtualRow.index} key="load-more">
                    <TableCell
                      colSpan={columns.length}
                      sx={{ py: 2, textAlign: 'center' }}
                    >
                      <LoadMoreButton
                        isLoading={isFetchingNextPage}
                        onLoadMore={onLoadMore}
                      />
                    </TableCell>
                  </TableRow>
                );
              }
              const row = rows[virtualRow.index];
              return (
                <TableRow
                  data-index={virtualRow.index}
                  key={row.id}
                  ref={rowVirtualizer.measureElement}
                >
                  {row.getVisibleCells().map((cell) => (
                    <TableCell key={cell.id}>
                      {flexRender(
                        cell.column.columnDef.cell,
                        cell.getContext(),
                      )}
                    </TableCell>
                  ))}
                </TableRow>
              );
            })}
            {paddingBottom > 0 && (
              <TableRow>
                <TableCell
                  colSpan={columns.length}
                  sx={{ height: paddingBottom, p: 0 }}
                />
              </TableRow>
            )}
          </TableBody>
        </Table>
      </TableContainer>
      {items.length === 0 && (
        <Typography sx={{ p: 2 }}>No entities matched this search.</Typography>
      )}
    </Paper>
  );
};
