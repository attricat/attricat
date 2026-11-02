import { useTranslation } from 'react-i18next';
import { LoadingSpinner } from '../../components/LoadingState';
import { resultsLoadingIndicatorSize } from './constants';

export const ExplorerLoadingIndicator = () => {
  const { t } = useTranslation();
  return (
    <LoadingSpinner
      label={t('explorer.loading')}
      size={resultsLoadingIndicatorSize}
      sx={{ alignItems: 'center', minHeight: '50vh' }}
    />
  );
};
