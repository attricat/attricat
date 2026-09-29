import { Box, Skeleton, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { extensionFrameSkeletonLines } from './constants';

/** Placeholder shown over a frame until its artifact reports readiness. */
export const ExtensionFrameLoading = () => {
  const { t } = useTranslation();
  return (
    <Box
      aria-live="polite"
      role="status"
      sx={{ inset: 0, p: 1, position: 'absolute' }}
    >
      {extensionFrameSkeletonLines.map((line) => (
        <Skeleton
          animation="wave"
          height={line.height}
          key={line.width}
          variant="text"
          width={line.width}
        />
      ))}
      <Typography sx={{ clip: 'rect(0 0 0 0)', position: 'absolute' }}>
        {t('extensions.loadingContent')}
      </Typography>
    </Box>
  );
};
