import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { AppsIcon } from '../../components/systemIcons';

export const AppsPage = () => {
  const { t } = useTranslation();

  return (
    <PageContainer>
      <PageHeader
        description={t('extensions.appsDescription')}
        icon={AppsIcon}
        title={t('extensions.appsTitle')}
      />
    </PageContainer>
  );
};
