import {
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';
import { blueprintExtensionContextVersion } from './constants';
import type { Blueprint } from './schemas';

export const PublishBlueprintDialog = ({
  blueprint,
  isPending,
  onClose,
  onConfirm,
  open,
}: {
  blueprint: Blueprint;
  isPending: boolean;
  onClose: () => void;
  onConfirm: () => void;
  open: boolean;
}) => {
  const { t } = useTranslation();
  return (
    <Dialog onClose={() => !isPending && onClose()} open={open}>
      <DialogTitle>{t('blueprints.publishBlueprintTitle')}</DialogTitle>
      <DialogContent>
        <DialogContentText>
          {t('blueprints.publishBlueprintDescription', {
            name: blueprint.name,
            version: blueprint.version,
          })}
        </DialogContentText>
        {open && (
          <Box sx={{ mt: 2 }}>
            <ExtensionOutlet
              context={{
                context_version: blueprintExtensionContextVersion,
                blueprint_id: blueprint.id,
                blueprint_version: blueprint.version,
              }}
              outlet="blueprint_publish_check"
              runtimeScope={{
                blueprintId: blueprint.id,
                blueprintVersion: blueprint.version,
              }}
            />
          </Box>
        )}
      </DialogContent>
      <DialogActions>
        <Button disabled={isPending} onClick={onClose}>
          {t('blueprints.cancel')}
        </Button>
        <Button disabled={isPending} onClick={onConfirm} variant="contained">
          {isPending ? t('blueprints.publishing') : t('blueprints.publish')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
