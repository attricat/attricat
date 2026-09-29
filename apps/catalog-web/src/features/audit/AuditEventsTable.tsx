import {
  Button,
  Chip,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { AuditEvent } from './api';
import { Timestamp } from '../../time/Timestamp';
import { auditActor, auditTarget } from './auditFormat';
import {
  auditOutcomeSuccess,
  auditTableColumnCount,
  executorTypes,
} from './constants';

export const AuditEventsTable = ({
  events,
  isLoading,
  onSelect,
  systemLabel,
  workspaceLabel,
}: {
  events?: AuditEvent[];
  isLoading: boolean;
  onSelect: (event: AuditEvent) => void;
  systemLabel: string;
  workspaceLabel: string;
}) => {
  const { t } = useTranslation();
  return (
    <Paper variant="outlined">
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>{t('audit.timestamp')}</TableCell>
            <TableCell>{t('audit.actor')}</TableCell>
            <TableCell>{t('audit.source')}</TableCell>
            <TableCell>{t('audit.action')}</TableCell>
            <TableCell>{t('audit.target')}</TableCell>
            <TableCell>{t('audit.outcome')}</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {events?.map((event) => (
            <TableRow hover key={event.id}>
              <TableCell>
                <Button
                  aria-label={t('audit.viewEvent', {
                    action: event.action,
                    actor: auditActor(event, systemLabel),
                  })}
                  onClick={() => onSelect(event)}
                  size="small"
                  variant="text"
                >
                  <Timestamp
                    focusable={false}
                    style="dateTimeSeconds"
                    value={event.occurred_at}
                  />
                </Button>
              </TableCell>
              <TableCell>{auditActor(event, systemLabel)}</TableCell>
              <TableCell>
                <Chip
                  color={
                    event.executor_type === executorTypes.agent
                      ? 'secondary'
                      : 'default'
                  }
                  label={event.executor_type}
                  size="small"
                />
              </TableCell>
              <TableCell>{event.action}</TableCell>
              <TableCell>{auditTarget(event, workspaceLabel)}</TableCell>
              <TableCell>
                <Chip
                  color={
                    event.outcome === auditOutcomeSuccess
                      ? 'success'
                      : 'default'
                  }
                  label={event.outcome}
                  size="small"
                />
              </TableCell>
            </TableRow>
          ))}
          {!isLoading && events?.length === 0 && (
            <TableRow>
              <TableCell colSpan={auditTableColumnCount}>
                {t('audit.noMatches')}
              </TableCell>
            </TableRow>
          )}
        </TableBody>
      </Table>
    </Paper>
  );
};
