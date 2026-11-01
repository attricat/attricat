import { Box, Button, MenuItem, TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { z } from 'zod';
import { AuditExecutorType } from './AuditExecutorType';
import type { AuditEventFilters } from './api';
import { zonedDateTimeToIso } from '../../time/instantFormat';
import { useTimeZone } from '../../time/useInstantFormat';
import { auditExecutorSelectMinWidth, executorTypes } from './constants';

/** Filters that name an event's actor, run or tool call by ID. */
const idFilterKeys = [
  'actor_user_id',
  'agent_run_id',
  'agent_tool_call_id',
] as const satisfies readonly (keyof AuditEventFilters)[];
type IdFilterKey = (typeof idFilterKeys)[number];

const isInvalidId = (value: string | undefined) =>
  value !== undefined && !z.uuid().safeParse(value).success;

/** Whether `To` comes before `From`, which no event can match. */
const isReversedRange = ({
  occurred_after,
  occurred_before,
}: AuditEventFilters) =>
  occurred_after !== undefined &&
  occurred_before !== undefined &&
  Date.parse(occurred_after) > Date.parse(occurred_before);

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
  const reversedRange = isReversedRange(draft);
  const invalid =
    reversedRange || idFilterKeys.some((key) => isInvalidId(draft[key]));
  // An ID the API cannot parse is reported here instead of sending it.
  const idFieldProps = (key: IdFilterKey) => ({
    error: isInvalidId(draft[key]),
    helperText: isInvalidId(draft[key]) ? t('audit.idInvalid') : undefined,
    onChange: (event: { target: { value: string } }) =>
      onChange(key, event.target.value.trim()),
  });
  return (
    <Box
      component="form"
      noValidate
      onSubmit={(event) => {
        event.preventDefault();
        if (!invalid) onApply();
      }}
      sx={{ display: 'flex', flexWrap: 'wrap', gap: 2, my: 3 }}
    >
      <TextField
        label={t('audit.from')}
        onChange={(event) =>
          onChange(
            'occurred_after',
            zonedDateTimeToIso(event.target.value, timeZone),
          )
        }
        slotProps={{ inputLabel: { shrink: true } }}
        type="datetime-local"
      />
      <TextField
        error={reversedRange}
        helperText={reversedRange ? t('audit.rangeReversed') : undefined}
        label={t('audit.to')}
        onChange={(event) =>
          onChange(
            'occurred_before',
            zonedDateTimeToIso(event.target.value, timeZone),
          )
        }
        slotProps={{ inputLabel: { shrink: true } }}
        type="datetime-local"
      />
      <TextField
        label={t('audit.actionCategory')}
        onChange={(event) => onChange('action_category', event.target.value)}
        placeholder={t('audit.actionCategoryPlaceholder')}
      />
      <TextField
        label={t('audit.actorId')}
        {...idFieldProps('actor_user_id')}
      />
      <TextField
        label={t('audit.targetType')}
        onChange={(event) => onChange('target_type', event.target.value)}
      />
      <TextField
        label={t('audit.executor')}
        onChange={(event) => onChange('executor_type', event.target.value)}
        select
        // Shows "All" rather than an empty field while no executor is chosen.
        slotProps={{
          inputLabel: { shrink: true },
          select: { displayEmpty: true },
        }}
        sx={{ minWidth: auditExecutorSelectMinWidth }}
        value={draft.executor_type ?? ''}
      >
        <MenuItem value="">{t('audit.all')}</MenuItem>
        <MenuItem value={executorTypes.human}>
          <AuditExecutorType executorType={executorTypes.human} />
        </MenuItem>
        <MenuItem value={executorTypes.agent}>
          <AuditExecutorType executorType={executorTypes.agent} />
        </MenuItem>
      </TextField>
      <TextField
        label={t('audit.agentRunId')}
        {...idFieldProps('agent_run_id')}
      />
      <TextField
        label={t('audit.toolCallId')}
        {...idFieldProps('agent_tool_call_id')}
      />
      <Button disabled={invalid} type="submit" variant="contained">
        {t('audit.applyFilters')}
      </Button>
    </Box>
  );
};
