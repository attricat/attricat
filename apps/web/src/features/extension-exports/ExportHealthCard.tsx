import { workspaceExportStatusOptions } from './queryOptions';
import { Paper, Stack, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { exportProblemStatuses, type ExportStatus } from './constants';
import { ExportStatusChip } from './ExportStatusChip';

/**
 * Export health on the Data Health page: per export extension, how many
 * records its destinations rejected, left uncertain or are retrying.
 */
export const ExportHealthCard = () => {
  const { t } = useTranslation();
  const overview = useQuery({
    ...workspaceExportStatusOptions(),
  });
  const destinations = overview.data?.destinations ?? [];
  const extensions = [
    ...new Set([
      ...destinations.map((destination) => destination.extension_id),
      ...(overview.data?.counts ?? []).map((count) => count.extension_id),
    ]),
  ].sort();
  if (!extensions.length) return null;
  return (
    <Paper component="section" sx={{ mt: 2, p: 2 }}>
      <Typography component="h2" variant="h6">
        {t('extensionExports.healthTitle')}
      </Typography>
      <Typography color="text.secondary" sx={{ mb: 1 }} variant="body2">
        {t('extensionExports.healthDescription')}
      </Typography>
      <Stack spacing={1}>
        {extensions.map((extensionId) => {
          const counts = (overview.data?.counts ?? []).filter(
            (count) => count.extension_id === extensionId,
          );
          const total = (status: ExportStatus) =>
            counts
              .filter((count) => count.status === status)
              .reduce((sum, count) => sum + count.count, 0);
          const title =
            destinations
              .filter((destination) => destination.extension_id === extensionId)
              .map((destination) => destination.title)
              .join(', ') || extensionId;
          return (
            <Stack
              direction={{ xs: 'column', sm: 'row' }}
              key={extensionId}
              spacing={1}
              sx={{ alignItems: { sm: 'center' } }}
            >
              <Link
                params={{ extensionId }}
                style={{ flex: 1 }}
                to="/manage/extensions/$extensionId"
              >
                {title}
              </Link>
              <Typography color="text.secondary" variant="body2">
                {t('extensionExports.syncedCount', { count: total('synced') })}
              </Typography>
              {exportProblemStatuses.map((status) =>
                total(status) ? (
                  <Stack
                    direction="row"
                    key={status}
                    spacing={0.5}
                    sx={{ alignItems: 'center' }}
                  >
                    <ExportStatusChip status={status} />
                    <Typography variant="body2">{total(status)}</Typography>
                  </Stack>
                ) : null,
              )}
            </Stack>
          );
        })}
      </Stack>
    </Paper>
  );
};
