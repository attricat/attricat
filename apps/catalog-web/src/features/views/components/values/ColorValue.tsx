import { Box, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { parseColor } from '../../colorValue';
import type { ValueRenderer } from '../componentTypes';

export const ColorValue: ValueRenderer = ({ value }) => {
  const { t } = useTranslation();
  const values = Array.isArray(value) && value.length ? value : [value];
  return (
    <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap' }}>
      {values.map((item, index) => {
        const color = parseColor(item);
        const missing =
          item == null || item === '' || (Array.isArray(item) && !item.length);
        return (
          <Stack
            key={index}
            direction="row"
            spacing={1}
            sx={{ alignItems: 'center' }}
          >
            {color && (
              <Box
                aria-hidden="true"
                sx={{
                  bgcolor: color,
                  border: 1,
                  borderColor: 'divider',
                  width: 20,
                  height: 20,
                  flexShrink: 0,
                }}
              />
            )}
            <Typography
              variant="body2"
              color={missing ? 'text.secondary' : undefined}
              sx={{ overflowWrap: 'anywhere' }}
            >
              {missing
                ? t('views.notSet')
                : typeof item === 'object'
                  ? JSON.stringify(item)
                  : String(item)}
            </Typography>
          </Stack>
        );
      })}
    </Stack>
  );
};
