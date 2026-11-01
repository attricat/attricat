import {
  Autocomplete,
  Box,
  Button,
  MenuItem,
  TextField,
  createFilterOptions,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { z } from 'zod';
import { AuditExecutorType } from './AuditExecutorType';
import type { AuditEventFilters } from './api';
import { zonedDateTimeToIso } from '../../time/instantFormat';
import { useTimeZone } from '../../time/useInstantFormat';
import {
  auditActorFieldMinWidth,
  auditExecutorSelectMinWidth,
  executorTypes,
} from './constants';
import { usePrincipalDirectory } from '../principals/usePrincipalDirectory';
import { userLabel } from '../principals/principal';
import type { DirectoryUser } from '../principals/schemas';

/** Filters that name an event's actor, run or tool call by ID. */
const idFilterKeys = [
  'actor_user_id',
  'agent_run_id',
  'agent_tool_call_id',
] as const satisfies readonly (keyof AuditEventFilters)[];
type IdFilterKey = (typeof idFilterKeys)[number];

const isInvalidId = (value: string | undefined) =>
  value !== undefined && !z.uuid().safeParse(value).success;

// People are found by name or email; former members stay listed.
const actorSearchText = (user: DirectoryUser) =>
  `${userLabel(user)} ${user.email}`;
const filterActors = createFilterOptions<DirectoryUser>({
  stringify: actorSearchText,
});

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
  const directory = usePrincipalDirectory();
  const actors = directory.data?.users ?? [];
  const actor = actors.find((user) => user.id === draft.actor_user_id);
  // Text that still finds someone is a search, not yet a wrong ID.
  const actorText = draft.actor_user_id?.toLowerCase();
  const invalidActor =
    !actor &&
    isInvalidId(draft.actor_user_id) &&
    !actors.some((user) =>
      actorSearchText(user)
        .toLowerCase()
        .includes(actorText ?? ''),
    );
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
      {/* Choosing a person fills in their ID; an ID can still be pasted,
          such as one from an event detail or of someone no longer listed. */}
      <Autocomplete
        filterOptions={filterActors}
        freeSolo
        getOptionLabel={(option) =>
          typeof option === 'string' ? option : userLabel(option)
        }
        onChange={(_, value) =>
          onChange(
            'actor_user_id',
            typeof value === 'string' ? value.trim() : (value?.id ?? ''),
          )
        }
        onInputChange={(_, value, reason) => {
          if (reason === 'input') onChange('actor_user_id', value.trim());
        }}
        options={actors}
        renderInput={(params) => (
          <TextField
            {...params}
            error={invalidActor}
            helperText={invalidActor ? t('audit.actorInvalid') : undefined}
            label={t('audit.actor')}
          />
        )}
        renderOption={({ key, ...props }, option) => (
          <li key={key} {...props}>
            {userLabel(option) === option.email
              ? option.email
              : `${userLabel(option)} · ${option.email}`}
          </li>
        )}
        sx={{ minWidth: auditActorFieldMinWidth }}
        value={actor ?? draft.actor_user_id ?? null}
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
