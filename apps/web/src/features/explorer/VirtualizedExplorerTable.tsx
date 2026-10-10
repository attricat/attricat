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
import type { RecordItem } from '../records/api';
import { displayLabel } from '../records/recordDisplay';
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
  table: ReturnType<typeof useLegacyTable<RecordItem>>;
  columnsLength: number;
  virtualRows: VirtualItem[];
  paddingTop: number;
  paddingBottom: number;
  measureElement: (element: HTMLTableRowElement | null) => void;
  tableContainerRef: React.Ref<HTMLDivElement>;
  selection: ExplorerSelection;
  items: RecordItem[];
  isFetching: boolean;
  isFetchingNextPage: boolean;
  onLoadMore: () => void;
  panelRecordId?: string;
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
  panelRecordId,
}: Props) => {
  const { t } = useTranslation();
  const rows = table.getRowModel().rows;
  const selectionHeader = (
    <Checkbox
      checked={selection.allLoadedSelected}
      disabled={items.length === 0}
      indeterminate={selection.someLoadedSelected}
      onChange={selection.toggleLoaded}
      slotProps={{
        input: {
          'aria-label': t('explorer.selectLoadedRecords', {
            limit: maximumAgentSelection,
          }),
        },
      }}
    />
  );
  const selectionCheckbox = (record: RecordItem) => (
    <Checkbox
      checked={selection.isSelected(record.id)}
      disabled={
        !selection.isSelected(record.id) &&
        selection.selectedItems.length >= maximumAgentSelection
      }
      onChange={() => selection.toggleRecord(record)}
      slotProps={{
        input: {
          'aria-label': t('explorer.selectRecord', {
            record: displayLabel(record.display, record.id),
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
                aria-current={row.id === panelRecordId || undefined}
                data-index={virtualRow.index}
                key={row.id}
                ref={measureElement}
                selected={row.id === panelRecordId}
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
