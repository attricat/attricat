import { Box, Button, MenuItem, TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { IconLabel } from '../../components/IconLabel';
import { AgentIcon, AssignedUserIcon } from '../../components/systemIcons';
import type { AuditEventFilters } from './api';
import { zonedDateTimeToIso } from '../../time/instantFormat';
import { useTimeZone } from '../../time/useInstantFormat';
import { executorTypes } from './constants';

export const AuditFilters = ({
  draft,
  onApply,
  onChange,
}: {
  draft: AuditEventFilters;
  onApply: () => void;
  onChange: (key: keyof AuditEventFilters, value: string) => void;
}) => {
  const { t } = useTranslation();
  // `datetime-local` values are wall-clock times in the user's zone.
  const timeZone = useTimeZone();
  return (
    <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 2, mb: 3 }}>
      <TextField
        label={t('audit.from')}
        onChange={(event) =>
          onChange(
            'occurred_after',
            zonedDateTimeToIso(event.target.value, timeZone),
          )
        }
        size="small"
        slotProps={{ inputLabel: { shrink: true } }}
        type="datetime-local"
      />
      <TextField
        label={t('audit.to')}
        onChange={(event) =>
          onChange(
            'occurred_before',
            zonedDateTimeToIso(event.target.value, timeZone),
          )
        }
        size="small"
        slotProps={{ inputLabel: { shrink: true } }}
        type="datetime-local"
      />
      <TextField
        label={t('audit.actionCategory')}
        onChange={(event) => onChange('action_category', event.target.value)}
        placeholder={t('audit.actionCategoryPlaceholder')}
        size="small"
      />
      <TextField
        label={t('audit.actorId')}
        onChange={(event) => onChange('actor_user_id', event.target.value)}
        size="small"
      />
      <TextField
        label={t('audit.targetType')}
        onChange={(event) => onChange('target_type', event.target.value)}
        size="small"
      />
      <TextField
        label={t('audit.executor')}
        onChange={(event) => onChange('executor_type', event.target.value)}
        select
        size="small"
        value={draft.executor_type ?? ''}
      >
        <MenuItem value="">{t('audit.all')}</MenuItem>
        <MenuItem value={executorTypes.human}>
          <IconLabel icon={AssignedUserIcon}>{t('audit.human')}</IconLabel>
        </MenuItem>
        <MenuItem value={executorTypes.agent}>
          <IconLabel icon={AgentIcon}>{t('audit.agent')}</IconLabel>
        </MenuItem>
      </TextField>
      <TextField
        label={t('audit.agentRunId')}
        onChange={(event) => onChange('agent_run_id', event.target.value)}
        size="small"
      />
      <TextField
        label={t('audit.toolCallId')}
        onChange={(event) => onChange('agent_tool_call_id', event.target.value)}
        size="small"
      />
      <Button onClick={onApply} variant="contained">
        {t('audit.applyFilters')}
      </Button>
    </Box>
  );
};
