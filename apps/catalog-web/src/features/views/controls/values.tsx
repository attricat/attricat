import { Box, Chip, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import {
  statusOptionLabel,
  type StatusConfiguration,
} from '../../entities/status';
import { MarkdownContent } from '../../markdown/MarkdownContent';
import type { ValueRenderer } from '../components/componentTypes';
import { NotSetValue } from '../components/values/NotSetValue';
import { parseColor } from './color';
import { EachValue } from './EachValue';
import { emailHref } from './email';
import { phoneHref } from './phone';
import { LinkedTextValue } from './TextControl';
import { safeUrl } from './url';

/** Opaque hex colors show a swatch beside the stored text. */
export const ColorValue: ValueRenderer = ({ value }) => (
  <EachValue
    value={value}
    direction="row"
    renderItem={(item) => {
      const color = parseColor(item);
      return (
        <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
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
          <Typography variant="body2" sx={{ overflowWrap: 'anywhere' }}>
            {typeof item === 'object' ? JSON.stringify(item) : String(item)}
          </Typography>
        </Stack>
      );
    }}
  />
);

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

export const MarkdownValue: ValueRenderer = ({ value }) =>
  typeof value === 'string' && value.trim() ? (
    <MarkdownContent value={value} />
  ) : (
    <NotSetValue />
  );

/** A status code as its labelled chip; unknown codes stay visible. */
export const StatusValue = ({
  config,
  value,
}: {
  config: StatusConfiguration;
  value: unknown;
}) => {
  const { t } = useTranslation();
  if (value === null || value === undefined) return <NotSetValue />;
  const option = config.options.find((item) => item.code === value);
  const label = option ? statusOptionLabel(option) : String(value);
  return (
    <Stack spacing={0.5} sx={{ alignItems: 'flex-start' }}>
      <Chip
        label={label}
        color={option?.tone ?? 'default'}
        variant="outlined"
        sx={{
          maxWidth: '100%',
          height: 'auto',
          '& .MuiChip-label': {
            whiteSpace: 'normal',
            overflowWrap: 'anywhere',
          },
        }}
      />
      {!option && (
        <Typography color="text.secondary" variant="caption">
          {t('entities.statusUnknown', { value: label })}
        </Typography>
      )}
    </Stack>
  );
};
