import { Box, CircularProgress } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { resultsLoadingIndicatorSize } from './constants';

export const ExplorerLoadingIndicator = () => {
  const { t } = useTranslation();
  return (
    <Box
      sx={{
        alignItems: 'center',
        display: 'flex',
        justifyContent: 'center',
        minHeight: '50vh',
      }}
    >
      <CircularProgress
        aria-label={t('explorer.loading')}
        enableTrackSlot
        size={resultsLoadingIndicatorSize}
      />
    </Box>
  );
};
