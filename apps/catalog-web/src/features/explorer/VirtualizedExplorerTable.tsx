import { flexRender } from '@tanstack/react-table';
import type { VirtualItem } from '@tanstack/react-virtual';
import type { useLegacyTable } from '@tanstack/react-table/legacy';
import {
  Box,
  Checkbox,
  LinearProgress,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  type SxProps,
  type Theme,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { LoadMoreButton } from '../../components/LoadMoreButton';
import type { EntityItem } from '../entities/api';
import { displayLabel } from '../entities/entityDisplay';
import {
  displayColumnMinWidth,
  explorerColumnIds,
  idColumnWidth,
  loadMoreRowKey,
  maximumAgentSelection,
  resultsTableHeight,
  stickyBodyLayer,
  stickyHeaderLayer,
} from './constants';
import type { ExplorerSelection } from './useExplorerSelection';

const resultColumnCellSx = (
  columnId: string,
  stickyLayer: number,
): SxProps<Theme> =>
  columnId === explorerColumnIds.actions
    ? {
        bgcolor: 'background.paper',
        boxShadow: 1,
        position: 'sticky',
        right: 0,
        zIndex: stickyLayer,
      }
    : columnId === explorerColumnIds.display
      ? { minWidth: displayColumnMinWidth }
      : columnId === explorerColumnIds.id
        ? { textAlign: 'center', width: idColumnWidth }
        : {};

type Props = {
  table: ReturnType<typeof useLegacyTable<EntityItem>>;
  columnsLength: number;
  virtualRows: VirtualItem[];
  paddingTop: number;
  paddingBottom: number;
  measureElement: (element: HTMLTableRowElement | null) => void;
  tableContainerRef: React.Ref<HTMLDivElement>;
  selection: ExplorerSelection;
  items: EntityItem[];
  isFetching: boolean;
  isFetchingNextPage: boolean;
  onLoadMore: () => void;
};

export const VirtualizedExplorerTable = ({
  table,
  columnsLength,
  virtualRows,
  paddingTop,
  paddingBottom,
  measureElement,
  tableContainerRef,
  selection,
  items,
  isFetching,
  isFetchingNextPage,
  onLoadMore,
}: Props) => {
  const { t } = useTranslation();
  const rows = table.getRowModel().rows;
  const selectionHeader = (
    <Checkbox
      checked={selection.allLoadedSelected}
      disabled={items.length === 0}
      indeterminate={
        selection.selectedItems.length > 0 && !selection.allLoadedSelected
      }
      onChange={selection.toggleLoaded}
      slotProps={{
        input: {
          'aria-label': t('explorer.selectLoadedEntities', {
            limit: maximumAgentSelection,
          }),
        },
      }}
    />
  );
  const selectionCheckbox = (entity: EntityItem) => (
    <Checkbox
      checked={selection.isSelected(entity.id)}
      disabled={
        !selection.isSelected(entity.id) &&
        selection.selectedItems.length >= maximumAgentSelection
      }
      onChange={() => selection.toggleEntity(entity.id)}
      slotProps={{
        input: {
          'aria-label': t('explorer.selectEntity', {
            entity: displayLabel(entity.display, entity.id),
          }),
        },
      }}
    />
  );

  return (
    <TableContainer
      ref={tableContainerRef}
      sx={{
        // On desktop this leaves room for the sticky search form and result
        // summary while using the rest of the viewport for rows.
        height: resultsTableHeight,
        overflowY: 'auto',
      }}
    >
      {isFetching && items.length > 0 && (
        <LinearProgress
          aria-label={t('explorer.loading')}
          sx={{ position: 'sticky', top: 0, zIndex: stickyHeaderLayer }}
        />
      )}
      <Table aria-label={t('explorer.results')} size="small" stickyHeader>
        <TableHead>
          {table.getHeaderGroups().map((group) => (
            <TableRow key={group.id}>
              {group.headers.map((header) => (
                <TableCell
                  key={header.id}
                  sx={resultColumnCellSx(header.column.id, stickyHeaderLayer)}
                >
                  {header.column.id === explorerColumnIds.select
                    ? selectionHeader
                    : header.isPlaceholder
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
                colSpan={columnsLength}
                sx={{ height: paddingTop, p: 0 }}
              />
            </TableRow>
          )}
          {virtualRows.map((virtualRow) => {
            if (virtualRow.index === rows.length) {
              return (
                <TableRow data-index={virtualRow.index} key={loadMoreRowKey}>
                  <TableCell colSpan={columnsLength} sx={{ py: 2 }}>
                    <Box
                      sx={{
                        left: '50%',
                        position: 'sticky',
                        transform: 'translateX(-50%)',
                        width: 'fit-content',
                      }}
                    >
                      <LoadMoreButton
                        isLoading={isFetchingNextPage}
                        onLoadMore={onLoadMore}
                      />
                    </Box>
                  </TableCell>
                </TableRow>
              );
            }
            const row = rows[virtualRow.index];
            return (
              <TableRow
                data-index={virtualRow.index}
                key={row.id}
                ref={measureElement}
              >
                {row.getVisibleCells().map((cell) => (
                  <TableCell
                    key={cell.id}
                    sx={resultColumnCellSx(cell.column.id, stickyBodyLayer)}
                  >
                    {cell.column.id === explorerColumnIds.select
                      ? selectionCheckbox(row.original)
                      : flexRender(
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
                colSpan={columnsLength}
                sx={{ height: paddingBottom, p: 0 }}
              />
            </TableRow>
          )}
        </TableBody>
      </Table>
    </TableContainer>
  );
};
