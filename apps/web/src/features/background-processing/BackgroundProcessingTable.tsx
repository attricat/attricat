import {
  Paper,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import {
  backgroundProcessingColumnKeys,
  noDueTaskPlaceholder,
  otherTaskKindTranslationKey,
  taskKindTranslationKeys,
} from './constants';
import type { BackgroundProcessingStatus } from './schemas';

export const BackgroundProcessingTable = ({
  rows,
}: {
  rows: BackgroundProcessingStatus;
}) => {
  const { t, i18n } = useTranslation();
  const number = new Intl.NumberFormat(i18n.language, {
    maximumFractionDigits: 0,
  });

  return (
    <TableContainer component={Paper}>
      <Table aria-label={t('backgroundProcessing.title')}>
        <TableHead>
          <TableRow>
            <TableCell>{t('backgroundProcessing.kind')}</TableCell>
            {backgroundProcessingColumnKeys.map((columnKey) => (
              <TableCell key={columnKey} align="right">
                {t(columnKey)}
              </TableCell>
            ))}
          </TableRow>
        </TableHead>
        <TableBody>
          {rows.map((row) => (
            <TableRow key={row.kind}>
              <TableCell component="th" scope="row">
                {t(
                  taskKindTranslationKeys[row.kind] ??
                    otherTaskKindTranslationKey,
                )}
              </TableCell>
              <TableCell align="right">{number.format(row.queued)}</TableCell>
              <TableCell align="right">{number.format(row.running)}</TableCell>
              <TableCell align="right">{number.format(row.failed)}</TableCell>
              <TableCell align="right">
                {number.format(row.expired_leases)}
              </TableCell>
              <TableCell align="right">
                {row.oldest_due_seconds === null
                  ? noDueTaskPlaceholder
                  : t('backgroundProcessing.seconds', {
                      value: number.format(row.oldest_due_seconds),
                    })}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </TableContainer>
  );
};
