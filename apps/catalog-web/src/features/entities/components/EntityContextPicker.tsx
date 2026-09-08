import { Box, MenuItem, Tab, Tabs, TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { AttributeContext } from '../api';

type EntityContextPickerProps = {
  contexts: readonly AttributeContext[];
  disabled?: boolean;
  onChange: (contextId: string) => void;
  value: string;
};

const tabContextLimit = 5;

/** Chooses an entity attribute context, prioritizing the first five as tabs. */
export const EntityContextPicker = ({
  contexts,
  disabled = false,
  onChange,
  value,
}: EntityContextPickerProps) => {
  const { t } = useTranslation();
  const tabContexts = contexts.slice(0, tabContextLimit);
  const remainingContexts = contexts.slice(tabContextLimit);
  const tabValue = tabContexts.some((context) => context.id === value)
    ? value
    : false;
  const remainingValue = remainingContexts.some(
    (context) => context.id === value,
  )
    ? value
    : '';

  return (
    <Box
      aria-label={t('entities.context')}
      component="nav"
      sx={{
        alignItems: 'center',
        borderBottom: 1,
        borderColor: 'divider',
        display: 'flex',
        gap: 1,
        mb: 2,
        mt: -1,
        width: '100%',
      }}
    >
      <Tabs
        aria-label={t('entities.context')}
        onChange={(_, contextId: string) => onChange(contextId)}
        sx={{ minHeight: 32 }}
        value={tabValue}
      >
        {tabContexts.map((context) => (
          <Tab
            disabled={disabled}
            key={context.id}
            label={
              context.code === 'default' ? t('entities.default') : context.code
            }
            sx={{ minHeight: 32, minWidth: 0, px: 1, py: 0.25 }}
            value={context.id}
          />
        ))}
      </Tabs>
      {remainingContexts.length > 0 && (
        <TextField
          select
          disabled={disabled}
          label={t('entities.moreContexts')}
          onChange={(event) => onChange(event.target.value)}
          size="small"
          sx={{ maxWidth: '100%', width: 180 }}
          value={remainingValue}
        >
          {remainingContexts.map((context) => (
            <MenuItem key={context.id} value={context.id}>
              {context.code === 'default'
                ? t('entities.default')
                : context.code}
            </MenuItem>
          ))}
        </TextField>
      )}
    </Box>
  );
};
