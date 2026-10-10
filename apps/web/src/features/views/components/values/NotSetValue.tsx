import { Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';

/** The muted label renderers show for an absent value. */
export const NotSetValue = () => {
  const { t } = useTranslation();
  return (
    <Typography color="text.secondary" variant="body2">
      {t('views.notSet')}
    </Typography>
  );
};
