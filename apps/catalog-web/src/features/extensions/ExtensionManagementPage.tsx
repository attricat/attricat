import { Link, type ToOptions } from '@tanstack/react-router';
import { Alert, Box, Button, Tab, Tabs } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/query-keys';
import { useQuery } from '@tanstack/react-query';

const extensionTabs = [
  { label: 'extensions.marketplace', to: '/manage/extensions/marketplace' },
  { label: 'extensions.installed', to: '/manage/extensions/installed' },
  { label: 'extensions.layout', to: '/manage/extensions/layout' },
] as const satisfies ReadonlyArray<{ label: string; to: ToOptions['to'] }>;

type ExtensionManagementPageProps = {
  children: ReactNode;
  tab: number;
};

export const ExtensionManagementPage = ({
  children,
  tab,
}: ExtensionManagementPageProps) => {
  const { t } = useTranslation();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const tabId = `extension-management-tab-${tab}`;
  const panelId = `extension-management-tabpanel-${tab}`;

  if (session.data && !session.data.capabilities?.extensions_read) {
    return (
      <PageContainer>
        <Alert severity="error">{t('extensions.notAuthorizedView')}</Alert>
      </PageContainer>
    );
  }

  return (
    <PageContainer>
      <PageHeader
        title={t('extensions.title')}
        description={t('extensions.description')}
        actions={
          session.data?.capabilities?.extensions_manage ? (
            <Button component={Link} to="/manage/extensions/sideload">
              {t('extensions.uploadArchive')}
            </Button>
          ) : undefined
        }
      />
      <Tabs aria-label={t('extensions.title')} sx={{ mb: 3 }} value={tab}>
        {extensionTabs.map((item, index) => (
          <Tab
            aria-controls={`extension-management-tabpanel-${index}`}
            component={Link}
            id={`extension-management-tab-${index}`}
            key={item.to}
            label={t(item.label)}
            to={item.to}
            value={index}
          />
        ))}
      </Tabs>
      <Box aria-labelledby={tabId} id={panelId} role="tabpanel">
        {children}
      </Box>
    </PageContainer>
  );
};
