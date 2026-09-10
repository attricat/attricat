import CloseIcon from '@mui/icons-material/Close';
import { Box, Drawer, IconButton, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { ExtensionOutlet } from '../../extensions/ExtensionOutlet';

export const EntityExtensionDrawer = ({
  contextId,
  entityId,
  onClose,
  open,
  showContent,
}: {
  contextId?: string;
  entityId: string;
  onClose: () => void;
  open: boolean;
  showContent: boolean;
}) => {
  const { t } = useTranslation();
  return (
    <Drawer anchor="right" onClose={onClose} open={open} variant="persistent">
      <Box
        id="entity-extension-contributions"
        sx={{ p: 3, width: { xs: '100vw', sm: 480 } }}
      >
        <Box sx={{ alignItems: 'center', display: 'flex' }}>
          <Typography sx={{ flexGrow: 1 }} variant="h6">
            {t('entities.extensionContributions')}
          </Typography>
          <IconButton
            aria-label={t('entities.closeExtensionContributions')}
            onClick={onClose}
          >
            <CloseIcon />
          </IconButton>
        </Box>
        {showContent && (
          <Box sx={{ mt: 2 }}>
            <ExtensionOutlet
              context={{ entity_id: entityId, context_id: contextId }}
              outlet="entity_preview_panel"
            />
          </Box>
        )}
      </Box>
    </Drawer>
  );
};
