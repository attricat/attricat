import { useState } from 'react';
import {
  Alert,
  Box,
  Button,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { ArrowRightIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import {
  bulkEditPreviewRows,
  formatBulkValue,
  type BulkEditProposal,
} from './bulkEdits';
import { BulkEditRecordLink } from './BulkEditRecordLink';

const BulkValue = ({ value }: { value: unknown }) => {
  const { t } = useTranslation();
  const text = formatBulkValue(value);
  return text === null ? (
    <Typography color="text.secondary" component="span" variant="body2">
      {t('agents.bulkEdit.noValue')}
    </Typography>
  ) : (
    <Box component="span" sx={{ overflowWrap: 'anywhere' }}>
      {text}
    </Box>
  );
};

/**
 * The records a bulk edit proposal changes, with each value's previous and
 * proposed state.
 */
export const BulkEditPreview = ({
  proposal,
}: {
  proposal: BulkEditProposal;
}) => {
  const { t } = useTranslation();
  const [showAll, setShowAll] = useState(false);
  const attributeName = new Map(
    proposal.attributes.map((attribute) => [attribute.code, attribute.name]),
  );
  const total = proposal.records.length;
  const records = showAll
    ? proposal.records
    : proposal.records.slice(0, bulkEditPreviewRows);
  return (
    <Stack spacing={1.5} sx={{ mt: 1.5 }}>
      <Alert severity="info">{t('agents.bulkEdit.failureNote')}</Alert>
      <TableContainer>
        <Table aria-label={t('agents.bulkEdit.preview')} size="small">
          <TableHead>
            <TableRow>
              <TableCell>{t('agents.bulkEdit.columns.record')}</TableCell>
              <TableCell>{t('agents.bulkEdit.columns.attribute')}</TableCell>
              <TableCell>{t('agents.bulkEdit.columns.change')}</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {records.flatMap((record) =>
              record.values.map((value, index) => (
                <TableRow key={`${record.record_id}:${index}`}>
                  {index === 0 && (
                    <TableCell
                      rowSpan={record.values.length}
                      sx={{ verticalAlign: 'top' }}
                    >
                      <BulkEditRecordLink
                        label={record.label}
                        recordId={record.record_id}
                      />
                    </TableCell>
                  )}
                  <TableCell>
                    {attributeName.get(value.attribute_code) ??
                      value.attribute_code}
                  </TableCell>
                  <TableCell>
                    <Stack
                      direction="row"
                      spacing={1}
                      sx={{ alignItems: 'center', flexWrap: 'wrap' }}
                    >
                      <BulkValue value={value.previous} />
                      <ArrowRightIcon
                        aria-label={t('agents.bulkEdit.changesTo')}
                        role="img"
                        size={compactIconSize}
                      />
                      <BulkValue value={value.value} />
                    </Stack>
                  </TableCell>
                </TableRow>
              )),
            )}
          </TableBody>
        </Table>
      </TableContainer>
      {total > bulkEditPreviewRows && (
        <Box>
          <Button onClick={() => setShowAll((current) => !current)}>
            {showAll
              ? t('agents.bulkEdit.showFirst', { count: bulkEditPreviewRows })
              : t('agents.bulkEdit.showAll', { count: total })}
          </Button>
        </Box>
      )}
    </Stack>
  );
};
