import {
  Alert,
  Button,
  FormControl,
  InputLabel,
  MenuItem,
  Select,
  Stack,
  TextField,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RefreshCwIcon } from 'lucide-react';
import {
  customThresholdInputWidth,
  customThresholdValue,
  maximumStaleAfterDays,
  minimumStaleAfterDays,
  staleAfterDayOptions,
  staleAfterLabelId,
  staleAfterSelectMinWidth,
} from './constants';

type Props = {
  staleAfterDays: number;
  isRefreshing: boolean;
  refreshError: Error | null;
  onStaleAfterDaysChange: (days: number) => void;
  onRefresh: () => void;
};

export const DataHealthControls = (props: Props) => (
  <DataHealthControlsForm key={props.staleAfterDays} {...props} />
);

const DataHealthControlsForm = ({
  staleAfterDays,
  isRefreshing,
  refreshError,
  onStaleAfterDaysChange,
  onRefresh,
}: Props) => {
  const { t } = useTranslation();
  const [customThreshold, setCustomThreshold] = useState(
    !staleAfterDayOptions.includes(
      staleAfterDays as (typeof staleAfterDayOptions)[number],
    ),
  );

  return (
    <>
      <Stack
        direction={{ xs: 'column', sm: 'row' }}
        spacing={2}
        sx={{ alignItems: { sm: 'center' } }}
      >
        <FormControl size="small" sx={{ minWidth: staleAfterSelectMinWidth }}>
          <InputLabel id={staleAfterLabelId}>
            {t('dataHealth.staleAfter')}
          </InputLabel>
          <Select
            label={t('dataHealth.staleAfter')}
            labelId={staleAfterLabelId}
            value={customThreshold ? customThresholdValue : staleAfterDays}
            onChange={(event) => {
              if (event.target.value === customThresholdValue) {
                setCustomThreshold(true);
                return;
              }
              setCustomThreshold(false);
              onStaleAfterDaysChange(Number(event.target.value));
            }}
          >
            {staleAfterDayOptions.map((days) => (
              <MenuItem key={days} value={days}>
                {t('dataHealth.daysCount', { count: days })}
              </MenuItem>
            ))}
            <MenuItem value={customThresholdValue}>
              {t('dataHealth.custom')}
            </MenuItem>
          </Select>
        </FormControl>
        {customThreshold && (
          <TextField
            defaultValue={staleAfterDays}
            label={t('dataHealth.days')}
            onBlur={(event) => {
              const days = Number(event.target.value);
              if (
                Number.isInteger(days) &&
                days >= minimumStaleAfterDays &&
                days <= maximumStaleAfterDays
              )
                onStaleAfterDaysChange(days);
            }}
            size="small"
            slotProps={{
              htmlInput: {
                min: minimumStaleAfterDays,
                max: maximumStaleAfterDays,
              },
            }}
            type="number"
            sx={{ width: customThresholdInputWidth }}
          />
        )}
        <Button
          disabled={isRefreshing}
          onClick={onRefresh}
          startIcon={<RefreshCwIcon />}
          variant="outlined"
        >
          {isRefreshing ? t('dataHealth.refreshing') : t('dataHealth.refresh')}
        </Button>
      </Stack>
      {refreshError && (
        <Alert severity="error" sx={{ mt: 2 }}>
          {refreshError.message}
        </Alert>
      )}
    </>
  );
};
