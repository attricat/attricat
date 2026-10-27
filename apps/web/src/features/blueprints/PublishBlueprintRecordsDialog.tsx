import { MenuItem, TextField } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ConfirmDialog } from '../../components/ConfirmDialog';
import { allPublicationChannels } from './constants';
import type { Blueprint } from './schemas';
import { lexiconText } from '../lexicon/lexicon';
import { publicationChannelsOptions } from '../exports/queryOptions';

export const PublishBlueprintRecordsDialog = ({
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
  const publicationChannels = useQuery(publicationChannelsOptions());

  return (
    <ConfirmDialog
      confirmLabel={
        isPending ? t('blueprints.publishing') : t('blueprints.publish')
      }
      error={error}
      isPending={isPending}
      onClose={onClose}
      onConfirm={() => onConfirm(publicationChannel)}
      open={open}
      title={t('blueprints.publishRecords')}
    >
      {t('blueprints.publishRecordsDescription', {
        name: lexiconText(blueprint.name),
        version: blueprint.version,
      })}
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
    </ConfirmDialog>
  );
};
