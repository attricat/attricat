import { Alert, Button, Stack } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { RouterButton } from '../../components/RouterLink';

type Props = {
  canReviewMigrations: boolean;
  capped: boolean;
  count: number;
  onShowAllVersions: () => void;
};

export const HiddenOutdatedNotice = ({
  canReviewMigrations,
  capped,
  count,
  onShowAllVersions,
}: Props) => {
  const { t } = useTranslation();
  return (
    <Alert
      action={
        <Stack direction="row" spacing={1}>
          <Button onClick={onShowAllVersions} size="small">
            {t('explorer.showAllVersions')}
          </Button>
          {canReviewMigrations && (
            <RouterButton size="small" to="/manage/data-health" variant="text">
              {t('explorer.reviewMigrations')}
            </RouterButton>
          )}
        </Stack>
      }
      severity="info"
      sx={{ mb: 2 }}
    >
      {capped
        ? t('explorer.hiddenOutdatedCapped', { count })
        : t('explorer.hiddenOutdated', { count })}
    </Alert>
  );
};
