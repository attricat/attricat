import { Box, Drawer, IconButton, Typography } from '@mui/material';
import { XIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { ExtensionOutlet } from '../../extensions/ExtensionOutlet';
import { RECORD_DRAWER_WIDTH, RECORD_EXTENSION_DRAWER_ID } from '../constants';

export const RecordExtensionDrawer = ({
  blueprintId,
  blueprintVersion,
  contextId,
  recordId,
  offset,
  onClose,
  open,
  showContent,
}: {
  blueprintId: string;
  blueprintVersion: number;
  contextId?: string;
  recordId: string;
  /** Right edge of the drawer, to open it beside another panel. */
  offset?: string;
  onClose: () => void;
  open: boolean;
  showContent: boolean;
}) => {
  const { t } = useTranslation();
  return (
    <Drawer
      anchor="right"
      onClose={onClose}
      open={open}
      slotProps={{ paper: { sx: { right: offset } } }}
      variant="persistent"
    >
      <Box
        id={RECORD_EXTENSION_DRAWER_ID}
        sx={{ p: 3, width: { xs: '100vw', sm: RECORD_DRAWER_WIDTH } }}
      >
        <Box sx={{ alignItems: 'center', display: 'flex' }}>
          <Typography sx={{ flexGrow: 1 }} variant="h6">
            {t('records.extensionContributions')}
          </Typography>
          <IconButton
            aria-label={t('records.closeExtensionContributions')}
            onClick={onClose}
          >
            <XIcon />
          </IconButton>
        </Box>
        {showContent && (
          <Box sx={{ mt: 2 }}>
            <ExtensionOutlet
              context={{ record_id: recordId, context_id: contextId }}
              outlet="record_preview_panel"
              runtimeScope={{ blueprintId, blueprintVersion }}
            />
          </Box>
        )}
      </Box>
    </Drawer>
  );
};
