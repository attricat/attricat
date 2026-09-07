import { Link } from '@tanstack/react-router';
import { flexRender } from '@tanstack/react-table';
import { useVirtualizer } from '@tanstack/react-virtual';
import {
  getCoreRowModel,
  legacyCreateColumnHelper,
  type LegacyColumnDef,
  useLegacyTable,
} from '@tanstack/react-table/legacy';
import TagOutlinedIcon from '@mui/icons-material/TagOutlined';
import {
  Box,
  Chip,
  IconButton,
  Popover,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Tooltip,
  Typography,
} from '@mui/material';
import { LoadMoreButton } from '../../components/LoadMoreButton';
import { useRef, useState } from 'react';
import type { BlueprintWithAttributes, EntityItem } from '../entities/api';
import { displayLabel } from '../entities/entity-display';
import { AttributeValue } from '../views/components/values/AttributeValue';

const EntityIdPopover = ({ entityId }: { entityId: string }) => {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const label = `View entity ID ${entityId}`;
  return (
    <>
      <Tooltip title="View entity ID">
        <IconButton
          aria-label={label}
          onClick={(event) => setAnchor(event.currentTarget)}
          size="small"
        >
          <TagOutlinedIcon fontSize="inherit" />
        </IconButton>
      </Tooltip>
      <Popover
        anchorEl={anchor}
        anchorOrigin={{ horizontal: 'left', vertical: 'bottom' }}
        onClose={() => setAnchor(null)}
        open={Boolean(anchor)}
      >
        <Typography
          component="code"
          sx={{ display: 'block', fontFamily: 'monospace', p: 2 }}
        >
          {entityId}
        </Typography>
      </Popover>
    </>
  );
};

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
      cell: (info) => <EntityIdPopover entityId={info.getValue()} />,
    }),
    columnHelper.display({
      id: 'display',
      header: 'Display',
      cell: (info) => (
        <Link
          params={{ entityId: info.row.original.id }}
          to="/entities/$entityId"
        >
          {displayLabel(info.row.original.display, info.row.original.id)}
        </Link>
      ),
    }) as LegacyColumnDef<EntityItem, string>,
    columnHelper.display({
      id: 'match-rationale',
      header: 'Matched by',
      cell: (info) => {
        const explanations = info.row.original.match_explanations;
        if (!explanations.length) return null;
        return (
          <Typography variant="body2">
            {explanations
              .map((explanation) =>
                explanation.traversal_depth
                  ? `${explanation.term} via ${explanation.traversal_depth} relationship${explanation.traversal_depth === 1 ? '' : 's'}`
                  : explanation.matching_attribute_code
                    ? `${explanation.term} in ${explanation.matching_attribute_code}`
                    : explanation.term,
              )
              .join('; ')}
          </Typography>
        );
      },
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
                  <TableCell
                    key={header.id}
                    sx={
                      header.column.id === 'display'
                        ? { minWidth: 280 }
                        : header.column.id === 'id'
                          ? { width: 56 }
                          : {}
                    }
                  >
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
                    <TableCell
                      key={cell.id}
                      sx={
                        cell.column.id === 'display'
                          ? { minWidth: 280 }
                          : cell.column.id === 'id'
                            ? { width: 56 }
                            : {}
                      }
                    >
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
