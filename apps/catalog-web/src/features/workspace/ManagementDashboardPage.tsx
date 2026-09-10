import AssessmentOutlinedIcon from '@mui/icons-material/AssessmentOutlined';
import BoltOutlinedIcon from '@mui/icons-material/BoltOutlined';
import CategoryOutlinedIcon from '@mui/icons-material/CategoryOutlined';
import FactCheckOutlinedIcon from '@mui/icons-material/FactCheckOutlined';
import FolderOutlinedIcon from '@mui/icons-material/FolderOutlined';
import ManageAccountsOutlinedIcon from '@mui/icons-material/ManageAccountsOutlined';
import { Box, Paper, Stack, Typography } from '@mui/material';
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';

const dashboardItems = [
  {
    descriptionKey: 'management.blueprintsDescription',
    icon: <CategoryOutlinedIcon color="primary" />,
    titleKey: 'navigation.blueprints',
    to: '/manage/blueprints',
  },
  {
    descriptionKey: 'management.contextsDescription',
    icon: <FolderOutlinedIcon color="primary" />,
    titleKey: 'navigation.contexts',
    to: '/manage/contexts',
  },
  {
    descriptionKey: 'management.dataHealthDescription',
    icon: <AssessmentOutlinedIcon color="primary" />,
    titleKey: 'navigation.dataHealth',
    to: '/manage/data-health',
  },
  {
    descriptionKey: 'management.workspaceDescription',
    icon: <ManageAccountsOutlinedIcon color="primary" />,
    titleKey: 'navigation.workspaceManagement',
    to: '/manage/workspace/members',
  },
  {
    descriptionKey: 'management.governanceDescription',
    icon: <FactCheckOutlinedIcon color="primary" />,
    titleKey: 'navigation.auditLog',
    to: '/manage/audit-log',
  },
  {
    descriptionKey: 'management.extensionsDescription',
    icon: <BoltOutlinedIcon color="primary" />,
    titleKey: 'navigation.extensions',
    to: '/manage/extensions',
  },
] as const;

export const ManagementDashboardPage = () => {
  const { t } = useTranslation();

  return (
    <PageContainer>
      <PageHeader
        description={t('management.dashboardDescription')}
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
                {item.icon}
                <Typography variant="h6">{t(item.titleKey)}</Typography>
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
