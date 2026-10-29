import { Box, MenuItem, Tab, Tabs, TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../../components/iconSizes';
import { ContextIcon } from '../../../components/systemIcons';
import type { AttributeContext } from '../../contexts/api';
import { defaultContextCode } from '../../contexts/constants';
import { contextLabel } from '../../contexts/defaultContext';
import {
  CONTEXT_MENU_WIDTH,
  CONTEXT_TAB_LIMIT,
  CONTEXT_TAB_MIN_HEIGHT,
} from '../constants';

type RecordContextPickerProps = {
  contexts: readonly AttributeContext[];
  disabled?: boolean;
  onChange: (contextId: string) => void;
  value: string;
};

/**
 * Chooses a record attribute context, prioritizing the first five as tabs.
 * The default context always comes first. Hidden while the workspace has only
 * its default context.
 */
export const RecordContextPicker = ({
  contexts,
  disabled = false,
  onChange,
  value,
}: RecordContextPickerProps) => {
  const { t } = useTranslation();
  if (contexts.length <= 1) return null;
  const orderedContexts = [
    ...contexts.filter((context) => context.code === defaultContextCode),
    ...contexts.filter((context) => context.code !== defaultContextCode),
  ];
  const tabContexts = orderedContexts.slice(0, CONTEXT_TAB_LIMIT);
  const remainingContexts = orderedContexts.slice(CONTEXT_TAB_LIMIT);
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
      aria-label={t('records.context')}
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
        aria-label={t('records.context')}
        onChange={(_, contextId: string) => onChange(contextId)}
        sx={{ minHeight: CONTEXT_TAB_MIN_HEIGHT }}
        value={tabValue}
      >
        {tabContexts.map((context) => (
          <Tab
            disabled={disabled}
            icon={<ContextIcon aria-hidden size={compactIconSize} />}
            key={context.id}
            label={contextLabel(t, context.code)}
            sx={{
              minHeight: CONTEXT_TAB_MIN_HEIGHT,
              minWidth: 0,
              px: 3,
              py: 0.25,
            }}
            value={context.id}
          />
        ))}
      </Tabs>
      {remainingContexts.length > 0 && (
        <TextField
          select
          disabled={disabled}
          label={t('records.moreContexts')}
          onChange={(event) => onChange(event.target.value)}
          sx={{ maxWidth: '100%', width: CONTEXT_MENU_WIDTH }}
          value={remainingValue}
        >
          {remainingContexts.map((context) => (
            <MenuItem key={context.id} value={context.id}>
              {contextLabel(t, context.code)}
            </MenuItem>
          ))}
        </TextField>
      )}
    </Box>
  );
};
