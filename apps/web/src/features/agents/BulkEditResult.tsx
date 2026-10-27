import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Box,
  Chip,
  Stack,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { apiErrorText } from '../../api/errors';
import { useRecordLabels } from '../records/useRecordLabels';
import type { BulkEditResult as BulkEditOutcome } from './schemas';
import { RecordIdLink } from '../../components/RecordIdLink';

const RecordList = ({
  ids,
  labels,
  title,
}: {
  ids: string[];
  labels: ReadonlyMap<string, string>;
  title: string;
}) =>
  ids.length > 0 && (
    <Accordion disableGutters elevation={0} sx={{ bgcolor: 'action.hover' }}>
      <AccordionSummary>{title}</AccordionSummary>
      <AccordionDetails>
        <Box component="ul" sx={{ m: 0, pl: 2.5 }}>
          {ids.map((id) => (
            <li key={id}>
              <RecordIdLink label={labels.get(id)} recordId={id} />
            </li>
          ))}
        </Box>
      </AccordionDetails>
    </Accordion>
  );

/** Which records an approved bulk edit applied, failed at and never ran. */
export const BulkEditResult = ({ result }: { result: BulkEditOutcome }) => {
  const { t } = useTranslation();
  const labels = useRecordLabels([
    ...result.applied,
    ...result.failed.map((failure) => failure.record_id),
    ...result.not_run,
  ]);
  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap' }}>
        <Chip
          color="success"
          label={t('agents.bulkEdit.applied', { count: result.applied.length })}
          size="small"
          variant="outlined"
        />
        {result.failed.length > 0 && (
          <Chip
            color="error"
            label={t('agents.bulkEdit.failed', { count: result.failed.length })}
            size="small"
            variant="outlined"
          />
        )}
        {result.not_run.length > 0 && (
          <Chip
            label={t('agents.bulkEdit.notRun', {
              count: result.not_run.length,
            })}
            size="small"
            variant="outlined"
          />
        )}
      </Stack>
      {result.failed.map((failure) => (
        <Box key={failure.record_id}>
          <Typography variant="body2">
            {t('agents.bulkEdit.stoppedAt')}{' '}
            <RecordIdLink
              label={labels.get(failure.record_id)}
              recordId={failure.record_id}
            />
          </Typography>
          <Typography color="text.secondary" variant="body2">
            {apiErrorText(failure)}
          </Typography>
        </Box>
      ))}
      <RecordList
        ids={result.applied}
        labels={labels}
        title={t('agents.bulkEdit.appliedRecords', {
          count: result.applied.length,
        })}
      />
      <RecordList
        ids={result.not_run}
        labels={labels}
        title={t('agents.bulkEdit.notRunRecords', {
          count: result.not_run.length,
        })}
      />
    </Stack>
  );
};
