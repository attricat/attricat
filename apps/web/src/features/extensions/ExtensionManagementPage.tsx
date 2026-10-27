import { Link, type ToOptions } from '@tanstack/react-router';
import { Box, Tab, Tabs } from '@mui/material';
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
import { useCan } from '../auth/useCan';
import { useTabAccessibility } from '../../components/useTabAccessibility';
import { ExtensionIcon } from '../../components/systemIcons';
import { capabilities } from '../auth/capabilities';
import { RequireCapability } from '../auth/RequireCapability';
import { RouterButton } from '../../components/RouterLink';

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
  const canManage = useCan(capabilities.extensionsManage);
  const tabId = useTabAccessibility();

  return (
    <RequireCapability capability={capabilities.extensionsRead}>
      <PageContainer>
        <PageHeader
          icon={ExtensionIcon}
          title={t('extensions.title')}
          description={t('extensions.description')}
          actions={
            canManage ? (
              <RouterButton
                startIcon={<UploadIcon />}
                to="/manage/extensions/sideload"
              >
                {t('extensions.uploadArchive')}
              </RouterButton>
            ) : undefined
          }
        />
        <Tabs aria-label={t('extensions.title')} sx={{ mb: 3 }} value={tab}>
          {extensionTabs.map((item, index) => (
            <Tab
              {...tabId.tab(index)}
              component={Link}
              icon={<item.icon aria-hidden size={compactIconSize} />}
              key={item.to}
              label={t(item.label)}
              to={item.to}
              value={index}
            />
          ))}
        </Tabs>
        <Box {...tabId.panel(tab)}>{children}</Box>
      </PageContainer>
    </RequireCapability>
  );
};
