import { Link, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { safeUrl } from '../urlPolicy';

export const UrlDisplay = ({ value }: { value: unknown }) => {
  const { t } = useTranslation();
  if (Array.isArray(value))
    return (
      <Stack spacing={0.5}>
        {value.map((item, index) => (
          <UrlDisplay key={index} value={item} />
        ))}
      </Stack>
    );
  const href = safeUrl(value);
  const text =
    value == null || value === ''
      ? t('views.notSet')
      : typeof value === 'string'
        ? value
        : JSON.stringify(value);
  return href ? (
    <Link
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      aria-label={t('views.urlOpenNewTab', { url: text })}
      onClick={(event) => event.stopPropagation()}
      onKeyDown={(event) => event.stopPropagation()}
      variant="body2"
      sx={{ overflowWrap: 'anywhere' }}
    >
      {text}
    </Link>
  ) : (
    <Typography variant="body2" sx={{ overflowWrap: 'anywhere' }}>
      {text}
    </Typography>
  );
};
