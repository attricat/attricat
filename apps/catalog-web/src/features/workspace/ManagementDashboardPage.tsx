import { createElement } from 'react';
import { Box, Paper, Stack, Typography, useTheme } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { getVisibleManagementNavigationItems } from '../../components/navigation';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { ManagementIcon } from '../../components/systemIcons';

export const ManagementDashboardPage = () => {
  const { t } = useTranslation();
  const theme = useTheme();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const dashboardItems = getVisibleManagementNavigationItems(
    session.data?.capabilities,
  );

  return (
    <PageContainer>
      <PageHeader
        description={t('management.dashboardDescription')}
        icon={ManagementIcon}
        title={t('management.dashboardTitle')}
      />
      <Box
        sx={{
          display: 'grid',
          gap: 2,
          gridTemplateColumns: { sm: 'repeat(2, minmax(0, 1fr))' },
          mt: 3,
        }}
      >
        {dashboardItems.map((item) => (
          <Link
            key={item.to}
            style={{ color: 'inherit', textDecoration: 'none' }}
            to={item.to}
          >
            <Paper
              sx={{
                height: '100%',
                p: 3,
                transition: 'box-shadow 150ms ease, transform 150ms ease',
                '&:hover': { boxShadow: 4, transform: 'translateY(-2px)' },
              }}
            >
              <Stack spacing={1.5}>
                {createElement(item.icon, {
                  color: theme.palette.primary.main,
                })}
                <Typography variant="h6">{t(item.labelKey)}</Typography>
                <Typography color="text.secondary">
                  {t(item.descriptionKey)}
                </Typography>
              </Stack>
            </Paper>
          </Link>
        ))}
      </Box>
    </PageContainer>
  );
};
