import { Box } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { ConfirmDialog } from '../../components/ConfirmDialog';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';
import type { Blueprint } from './schemas';
import { lexiconText } from '../lexicon/lexicon';
import {
  extensionOutlets,
  supportedOutletContextVersion,
} from '../extensions/constants';

export const PublishBlueprintDialog = ({
  blueprint,
  error,
  isPending,
  onClose,
  onConfirm,
  open,
}: {
  blueprint: Blueprint;
  error: Error | null;
  isPending: boolean;
  onClose: () => void;
  onConfirm: () => void;
  open: boolean;
}) => {
  const { t } = useTranslation();
  return (
    <ConfirmDialog
      confirmLabel={
        isPending ? t('blueprints.publishing') : t('blueprints.publish')
      }
      error={error}
      isPending={isPending}
      maxWidth="sm"
      onClose={onClose}
      onConfirm={onConfirm}
      open={open}
      title={t('blueprints.publishBlueprintTitle')}
    >
      {t('blueprints.publishBlueprintDescription', {
        name: lexiconText(blueprint.name),
        version: blueprint.version,
      })}
      {open && (
        <Box sx={{ mt: 2 }}>
          <ExtensionOutlet
            context={{
              context_version: supportedOutletContextVersion,
              blueprint_id: blueprint.id,
              blueprint_version: blueprint.version,
            }}
            outlet={extensionOutlets.blueprintPublishCheck}
            runtimeScope={{
              blueprintId: blueprint.id,
              blueprintVersion: blueprint.version,
            }}
          />
        </Box>
      )}
    </ConfirmDialog>
  );
};
