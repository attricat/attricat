import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';

export const AppsPage = () => {
  const { t } = useTranslation();

  return (
    <PageContainer>
      <PageHeader
        description={t('extensions.appsDescription')}
        title={t('extensions.appsTitle')}
      />
    </PageContainer>
  );
};
