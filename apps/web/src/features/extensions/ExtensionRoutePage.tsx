import { Alert, Box, CircularProgress } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { ExtensionFrame } from './ExtensionFrame';
import { useExtensionRuntime } from './useExtensionRuntime';

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
      <Box
        aria-live="polite"
        role="status"
        sx={{ display: 'flex', justifyContent: 'center', py: 2 }}
      >
        <CircularProgress
          aria-label={t('extensions.loadingPage')}
          enableTrackSlot
        />
      </Box>
    );
  if (runtime.isError)
    return (
      <Alert severity="warning">{t('extensions.contentLoadFailed')}</Alert>
    );
  const contribution = runtime.data?.find(
    (item) =>
      item.kind === 'route' &&
      item.extension_id === extensionId &&
      item.id === contributionId,
  );
  if (!contribution)
    return <Alert severity="warning">{t('extensions.pageUnavailable')}</Alert>;
  return <ExtensionFrame contribution={contribution} />;
};
