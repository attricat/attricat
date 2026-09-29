import { Box, Fab } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { InspectorIcon } from '../../components/systemIcons';
import {
  apiHealthColor,
  apiHealthSummaryKeys,
  type ApiHealthState,
} from './apiHealth';
import { launcherStatusDotOffset, launcherStatusDotSize } from './constants';

/** Collapsed Inspector button with a small API health indicator. */
export const InspectorLauncher = ({
  apiHealth,
  onOpen,
}: {
  apiHealth: ApiHealthState;
  onOpen: () => void;
}) => {
  const { t } = useTranslation();
  return (
    <Fab
      aria-label={t('inspector.open')}
      color="default"
      onClick={onOpen}
      size="small"
      sx={{
        backgroundColor: 'background.paper',
        bottom: (theme) => theme.spacing(4),
        color: 'text.secondary',
        left: '50%',
        position: 'fixed',
        transform: 'translateX(-50%)',
        zIndex: (theme) => theme.zIndex.modal + 1,
        '&:hover': {
          backgroundColor: 'action.hover',
          boxShadow: 4,
          color: 'primary.main',
        },
      }}
    >
      <InspectorIcon fontSize="small" />
      <Box
        aria-label={t(apiHealthSummaryKeys[apiHealth])}
        role="status"
        sx={{
          backgroundColor: apiHealthColor(apiHealth),
          border: 2,
          borderColor: 'background.paper',
          borderRadius: '50%',
          bottom: launcherStatusDotOffset,
          height: launcherStatusDotSize,
          position: 'absolute',
          right: launcherStatusDotOffset,
          width: launcherStatusDotSize,
        }}
      />
    </Fab>
  );
};
