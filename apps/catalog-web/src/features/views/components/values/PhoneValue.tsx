import { Link, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { phoneHref } from '../../phoneNumber';
import type { ValueRenderer } from '../componentTypes';
import { AttributeValue } from './AttributeValue';

export const PhoneValue: ValueRenderer = ({ attribute, value }) => {
  const { t } = useTranslation();
  if (typeof value !== 'string')
    return <AttributeValue attribute={attribute} value={value} />;
  if (!value.trim())
    return (
      <Typography color="text.secondary" variant="body2">
        {t('views.notSet')}
      </Typography>
    );
  const href = phoneHref(value);
  return href ? (
    <Link
      href={href}
      aria-label={t('views.callPhone', { number: value })}
      dir="ltr"
      variant="body2"
      sx={{ overflowWrap: 'anywhere', unicodeBidi: 'isolate' }}
      onClick={(event) => event.stopPropagation()}
      onKeyDown={(event) => event.stopPropagation()}
    >
      {value}
    </Link>
  ) : (
    <Typography
      dir="ltr"
      variant="body2"
      sx={{ overflowWrap: 'anywhere', unicodeBidi: 'isolate' }}
    >
      {value}
    </Typography>
  );
};
