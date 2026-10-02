import { Box, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { MarkdownContent } from '../../markdown/MarkdownContent';
import type { ValueRenderer } from '../components/componentTypes';
import { parseColor } from './color';
import { emailHref } from './email';
import { phoneHref } from './phone';
import { LinkedTextValue } from './TextControl';
import { safeUrl } from './url';

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

export const EmailValue: ValueRenderer = ({ value }) => (
  <LinkedTextValue value={value} href={emailHref} />
);

export const UrlValue: ValueRenderer = ({ value }) => {
  const { t } = useTranslation();
  return (
    <LinkedTextValue
      value={value}
      href={safeUrl}
      linkProps={(url) => ({
        target: '_blank',
        rel: 'noopener noreferrer',
        'aria-label': t('views.urlOpenNewTab', { url }),
      })}
    />
  );
};

export const PhoneValue: ValueRenderer = ({ value }) => {
  const { t } = useTranslation();
  return (
    <LinkedTextValue
      value={value}
      href={phoneHref}
      leftToRight
      linkProps={(number) => ({
        'aria-label': t('views.callPhone', { number }),
      })}
    />
  );
};

export const MarkdownValue: ValueRenderer = ({ value }) => {
  const { t } = useTranslation();
  return typeof value === 'string' && value.trim() ? (
    <MarkdownContent value={value} />
  ) : (
    <Typography color="text.secondary" variant="body2">
      {t('views.notSet')}
    </Typography>
  );
};
