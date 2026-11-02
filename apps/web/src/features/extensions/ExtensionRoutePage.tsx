import { Alert } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { ExtensionFrame } from './ExtensionFrame';
import { useExtensionRuntime } from './useExtensionRuntime';
import { contributionKinds } from './constants';
import { LoadingSpinner } from '../../components/LoadingState';

type ExtensionRoutePageProps = {
  extensionId: string;
  contributionId: string;
};

export const ExtensionRoutePage = ({
  extensionId,
  contributionId,
}: ExtensionRoutePageProps) => {
  const { t } = useTranslation();
  const runtime = useExtensionRuntime();
  if (runtime.isPending)
    return (
      <LoadingSpinner label={t('extensions.loadingPage')} sx={{ py: 2 }} />
    );
  if (runtime.isError)
    return (
      <Alert severity="warning">{t('extensions.contentLoadFailed')}</Alert>
    );
  const contribution = runtime.data?.find(
    (item) =>
      item.kind === contributionKinds.route &&
      item.extension_id === extensionId &&
      item.id === contributionId,
  );
  if (!contribution)
    return <Alert severity="warning">{t('extensions.pageUnavailable')}</Alert>;
  return <ExtensionFrame contribution={contribution} />;
};
