import { useQuery } from '@tanstack/react-query';
import { Link, type ToOptions } from '@tanstack/react-router';
import { Alert, Box, Button, Tab, Tabs } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import {
  PackageCheckIcon,
  PanelsTopLeftIcon,
  StoreIcon,
  UploadIcon,
  type LucideIcon,
} from 'lucide-react';
import { compactIconSize } from '../../components/iconSizes';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { extensionManagementTabIds } from './constants';
import { ExtensionIcon } from '../../components/systemIcons';

const extensionTabs = [
  {
    icon: StoreIcon,
    label: 'extensions.marketplace',
    to: '/manage/extensions/marketplace',
  },
  {
    icon: PackageCheckIcon,
    label: 'extensions.installed',
    to: '/manage/extensions/installed',
  },
  {
    icon: PanelsTopLeftIcon,
    label: 'extensions.layout',
    to: '/manage/extensions/layout',
  },
] as const satisfies ReadonlyArray<{
  icon: LucideIcon;
  label: string;
  to: ToOptions['to'];
}>;

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
        icon={ExtensionIcon}
        title={t('extensions.title')}
        description={t('extensions.description')}
        actions={
          session.data?.capabilities?.extensions_manage ? (
            <Button
              component={Link}
              startIcon={<UploadIcon />}
              to="/manage/extensions/sideload"
            >
              {t('extensions.uploadArchive')}
            </Button>
          ) : undefined
        }
      />
      <Tabs aria-label={t('extensions.title')} sx={{ mb: 3 }} value={tab}>
        {extensionTabs.map((item, index) => (
          <Tab
            aria-controls={extensionManagementTabIds.panel(index)}
            component={Link}
            icon={<item.icon aria-hidden size={compactIconSize} />}
            id={extensionManagementTabIds.tab(index)}
            key={item.to}
            label={t(item.label)}
            to={item.to}
            value={index}
          />
        ))}
      </Tabs>
      <Box
        aria-labelledby={extensionManagementTabIds.tab(tab)}
        id={extensionManagementTabIds.panel(tab)}
        role="tabpanel"
      >
        {children}
      </Box>
    </PageContainer>
  );
};
