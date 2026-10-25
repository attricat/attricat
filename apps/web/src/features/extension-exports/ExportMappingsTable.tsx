import {
  Checkbox,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
} from '@mui/material';
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { Timestamp } from '../../time/Timestamp';
import { ExportStatusChip } from './ExportStatusChip';
import { formatPushError } from './pushErrors';
import type { ExportMapping } from './schemas';

/**
 * Records in one export status with their push errors; with `selected`,
 * each row can be picked for a retry.
 */
export const ExportMappingsTable = ({
  mappings,
  onSelectedChange,
  selected,
}: {
  mappings: ExportMapping[];
  onSelectedChange: (update: (current: string[]) => string[]) => void;
  /** The picked record IDs, or `undefined` when rows cannot be picked. */
  selected: string[] | undefined;
}) => {
  const { t } = useTranslation();
  return (
    <Table size="small">
      <TableHead>
        <TableRow>
          {selected && <TableCell padding="checkbox" />}
          <TableCell>{t('extensionExports.record')}</TableCell>
          <TableCell>{t('extensionExports.statusColumn')}</TableCell>
          <TableCell>{t('extensionExports.error')}</TableCell>
          <TableCell>{t('extensionExports.updated')}</TableCell>
        </TableRow>
      </TableHead>
      <TableBody>
        {mappings.map((mapping) => (
          <TableRow key={mapping.record_id}>
            {selected && (
              <TableCell padding="checkbox">
                <Checkbox
                  checked={selected.includes(mapping.record_id)}
                  slotProps={{
                    input: {
                      'aria-label': t('extensionExports.selectRecord', {
                        record: mapping.record_id,
                      }),
                    },
                  }}
                  onChange={(event) =>
                    onSelectedChange((current) =>
                      event.target.checked
                        ? [...current, mapping.record_id]
                        : current.filter((id) => id !== mapping.record_id),
                    )
                  }
                />
              </TableCell>
            )}
            <TableCell sx={{ overflowWrap: 'anywhere' }}>
              <Link
                params={{ recordId: mapping.record_id }}
                to="/records/$recordId"
              >
                {mapping.external_id ?? mapping.record_id}
              </Link>
            </TableCell>
            <TableCell>
              <ExportStatusChip status={mapping.status} />
            </TableCell>
            <TableCell>{formatPushError(mapping.error) || '—'}</TableCell>
            <TableCell>
              <Timestamp value={mapping.updated_at} />
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
};
