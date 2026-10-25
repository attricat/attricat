import {
  Box,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { exportStatuses } from './constants';
import { allConnections, type ConnectionCounts } from './exportCounts';

/** How many records each connection has in each export status. */
export const ExportCountsTable = ({ rows }: { rows: ConnectionCounts[] }) => {
  const { t } = useTranslation();
  return (
    <Box sx={{ overflowX: 'auto' }}>
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>{t('extensionExports.connection')}</TableCell>
            {exportStatuses.map((item) => (
              <TableCell align="right" key={item}>
                {t(`extensionExports.status.${item}`)}
              </TableCell>
            ))}
          </TableRow>
        </TableHead>
        <TableBody>
          {rows.map((row) => (
            <TableRow key={row.connection ?? allConnections}>
              <TableCell>
                {row.connection ?? t('extensionExports.defaultConnection')}
              </TableCell>
              {exportStatuses.map((item) => (
                <TableCell align="right" key={item}>
                  {row.counts[item] ?? 0}
                </TableCell>
              ))}
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Box>
  );
};
