import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Autocomplete,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useEffect, useMemo, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { updatePreferences } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import {
  browserTimeZone,
  supportedTimeZones,
  timeZoneLabel,
} from '../../time/instantFormat';
import { Timestamp } from '../../time/Timestamp';
import { usePreferredTimeZone } from '../../time/useInstantFormat';
import {
  previewRefreshMilliseconds,
  timeZonePickerMaxWidth,
} from './constants';

type TimeZoneOption = {
  /** `null` follows the browser zone. */
  value: string | null;
  label: string;
};

/** Stores the account time zone used by every rendered timestamp. */
export const TimeZonePreference = ({ disabled }: { disabled: boolean }) => {
  const { i18n, t } = useTranslation();
  const client = useQueryClient();
  const preferred = usePreferredTimeZone();
  const locale = i18n.resolvedLanguage ?? i18n.language;
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const interval = window.setInterval(
      () => setNow(new Date()),
      previewRefreshMilliseconds,
    );
    return () => window.clearInterval(interval);
  }, []);
  const save = useMutation({
    mutationFn: (timeZone: string | null) =>
      updatePreferences({ time_zone: timeZone }),
    onSuccess: (session) =>
      client.setQueryData(authQueryKeys.session(), session),
  });
  const options = useMemo<TimeZoneOption[]>(() => {
    // Labels include the current offset, fixed for the page lifetime.
    const labelledAt = new Date();
    return [
      {
        value: null,
        label: t('timeZone.automatic', { zone: browserTimeZone() }),
      },
      ...supportedTimeZones().map((zone) => ({
        value: zone,
        label: `${zone} (${timeZoneLabel(labelledAt, locale, zone)})`,
      })),
    ];
  }, [locale, t]);
  const selected =
    options.find((option) => option.value === preferred) ??
    // A stored zone this browser does not know is still shown as selected.
    ({ value: preferred, label: preferred ?? '' } satisfies TimeZoneOption);

  return (
    <Stack spacing={2}>
      <Typography variant="h6">{t('timeZone.label')}</Typography>
      <Typography color="text.secondary" variant="body2">
        {t('timeZone.description')}
      </Typography>
      {save.isError && (
        <Alert severity="error">
          {t('timeZone.saveFailed', { message: save.error.message })}
        </Alert>
      )}
      <Autocomplete
        autoHighlight
        disableClearable
        disabled={disabled || save.isPending}
        getOptionKey={(option) => option.value ?? ''}
        isOptionEqualToValue={(option, value) => option.value === value.value}
        noOptionsText={t('timeZone.noOptions')}
        onChange={(_, option) => {
          if (option.value !== preferred) save.mutate(option.value);
        }}
        options={options}
        renderInput={(params) => (
          <TextField {...params} label={t('timeZone.label')} />
        )}
        sx={{ maxWidth: timeZonePickerMaxWidth, width: '100%' }}
        value={selected}
      />
      <Typography variant="body2">
        <Trans
          components={{ timestamp: <Timestamp style="dateTime" value={now} /> }}
          i18nKey="timeZone.preview"
          t={t}
        />
      </Typography>
    </Stack>
  );
};
