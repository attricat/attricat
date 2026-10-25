import {
  Box,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { Timestamp } from '../../time/Timestamp';
import { formatPushError } from './pushErrors';
import type { HandlerFailure } from './schemas';

/** Event deliveries to the extension's handlers that keep failing. */
export const HandlerFailuresTable = ({
  failures,
}: {
  failures: HandlerFailure[];
}) => {
  const { t } = useTranslation();
  return (
    <Box sx={{ mt: 3 }}>
      <Typography variant="subtitle1">
        {t('extensionExports.handlerFailures')}
      </Typography>
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>{t('extensionExports.event')}</TableCell>
            <TableCell>{t('extensionExports.error')}</TableCell>
            <TableCell>{t('extensionExports.attempts')}</TableCell>
            <TableCell>{t('extensionExports.updated')}</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {failures.map((failure) => (
            <TableRow key={`${failure.event_id}:${failure.handler_id}`}>
              <TableCell>
                {failure.event_type} · {failure.handler_id}
              </TableCell>
              <TableCell>
                {formatPushError({
                  code: failure.error_code,
                  message: failure.error_message,
                })}
              </TableCell>
              <TableCell>{failure.attempts}</TableCell>
              <TableCell>
                <Timestamp value={failure.updated_at} />
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </Box>
  );
};
