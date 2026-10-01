import { Link, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { emailHref } from '../../email';

/** Also accepts multi-valued relationship projections without fetching target rows. */
export const EmailValue = ({ value }: { value: unknown }) => {
  const { t } = useTranslation();
  if (Array.isArray(value))
    return (
      <Stack spacing={0.5}>
        {value.length ? (
          value.map((item, index) => (
            <EmailValue
              key={index}
              value={Array.isArray(item) ? JSON.stringify(item) : item}
            />
          ))
        ) : (
          <EmailValue value={null} />
        )}
      </Stack>
    );
  const missing = value == null || value === '';
  const href = emailHref(value);
  const text = missing
    ? t('views.notSet')
    : typeof value === 'object'
      ? JSON.stringify(value)
      : String(value);
  return href ? (
    <Link
      href={href}
      onClick={(event) => event.stopPropagation()}
      onKeyDown={(event) => event.stopPropagation()}
      sx={{ overflowWrap: 'anywhere' }}
      variant="body2"
    >
      {text}
    </Link>
  ) : (
    <Typography
      color={missing ? 'text.secondary' : undefined}
      sx={{ overflowWrap: 'anywhere' }}
      variant="body2"
    >
      {text}
    </Typography>
  );
};
