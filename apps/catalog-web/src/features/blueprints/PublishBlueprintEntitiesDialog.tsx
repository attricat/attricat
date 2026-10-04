import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  MenuItem,
  TextField,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../components/CheckViolationsAlert';
import { listPublicationChannels } from '../exports/api';
import { exportQueryKeys } from '../exports/queryKeys';
import { allPublicationChannels } from './constants';
import type { Blueprint } from './schemas';
import { lexiconText } from '../lexicon/lexicon';

export const PublishBlueprintEntitiesDialog = ({
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
  onConfirm: (contextId: string) => void;
  open: boolean;
}) => {
  const { t } = useTranslation();
  const [publicationChannel, setPublicationChannel] = useState<string>(
    allPublicationChannels,
  );
  const publicationChannels = useQuery({
    queryKey: exportQueryKeys.channels(),
    queryFn: listPublicationChannels,
  });

  return (
    <Dialog onClose={() => !isPending && onClose()} open={open}>
      <DialogTitle>{t('blueprints.publishEntities')}</DialogTitle>
      <DialogContent>
        <DialogContentText>
          {t('blueprints.publishEntitiesDescription', {
            name: lexiconText(blueprint.name),
            version: blueprint.version,
          })}
        </DialogContentText>
        <TextField
          fullWidth
          label={t('blueprints.publicationChannel')}
          onChange={(event) => setPublicationChannel(event.target.value)}
          select
          sx={{ mt: 2 }}
          value={publicationChannel}
        >
          <MenuItem value={allPublicationChannels}>
            {t('blueprints.allEnabledChannels')}
          </MenuItem>
          {publicationChannels.data
            ?.filter((channel) => channel.enabled)
            .map((channel) => (
              <MenuItem key={channel.context_id} value={channel.context_id}>
                {channel.context_code}
              </MenuItem>
            ))}
        </TextField>
        {error && <ApiErrorAlert error={error} sx={{ mt: 2 }} />}
      </DialogContent>
      <DialogActions>
        <Button disabled={isPending} onClick={onClose}>
          {t('blueprints.cancel')}
        </Button>
        <Button
          disabled={isPending}
          onClick={() => onConfirm(publicationChannel)}
          variant="contained"
        >
          {isPending ? t('blueprints.publishing') : t('blueprints.publish')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
