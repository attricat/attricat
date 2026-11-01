import { Button, Stack, TextField } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { UseMutationResult } from '@tanstack/react-query';
import {
  PencilIcon,
  PlayIcon,
  PowerIcon,
  PowerOffIcon,
  SendIcon,
} from 'lucide-react';
import { z } from 'zod';
import { RouterButton } from '../../components/RouterLink';
import { workflowRoutes } from './constants';
import { revisionStatuses } from '../../api/revisionStatus';
import type { Workflow } from './schemas';

type VersionMutation = UseMutationResult<unknown, Error, number>;

export const WorkflowActions = ({
  current,
  disable,
  enable,
  publish,
  runNow,
  workflowId,
}: {
  current: Workflow;
  disable: UseMutationResult<unknown, Error, void>;
  enable: VersionMutation;
  publish: VersionMutation;
  runNow: UseMutationResult<unknown, Error, string>;
  workflowId: string;
}) => {
  const { t } = useTranslation();
  const [manualRecordId, setManualRecordId] = useState('');
  const [recordIdInvalid, setRecordIdInvalid] = useState(false);
  const isEnabledRevision = current.enabled_version === current.version;
  return (
    <Stack
      direction="row"
      spacing={1}
      sx={{ alignItems: 'flex-start', flexWrap: 'wrap' }}
    >
      <RouterButton
        params={{ workflowId, version: String(current.version) }}
        startIcon={<PencilIcon />}
        to={workflowRoutes.newRevision}
        variant="outlined"
      >
        {t('workflows.newRevision')}
      </RouterButton>
      {current.status === revisionStatuses.draft && (
        <Button
          disabled={publish.isPending}
          onClick={() => publish.mutate(current.version)}
          startIcon={<SendIcon />}
          variant="contained"
        >
          {t('workflows.publish')}
        </Button>
      )}
      {current.status === revisionStatuses.published && !isEnabledRevision && (
        <Button
          disabled={enable.isPending}
          onClick={() => enable.mutate(current.version)}
          startIcon={<PowerIcon />}
          variant="contained"
        >
          {t('workflows.enable')}
        </Button>
      )}
      {isEnabledRevision && current.manual_enabled && (
        <Stack direction="row" spacing={1} sx={{ alignItems: 'flex-start' }}>
          <TextField
            disabled={runNow.isPending}
            error={recordIdInvalid}
            helperText={
              recordIdInvalid ? t('workflows.invalidRecordId') : undefined
            }
            label={t('workflows.recordId')}
            required
            size="small"
            slotProps={{
              htmlInput: { 'aria-label': t('workflows.manualRunRecordId') },
            }}
            onChange={(event) => {
              setManualRecordId(event.target.value);
              setRecordIdInvalid(false);
            }}
            value={manualRecordId}
          />
          <Button
            disabled={runNow.isPending || !manualRecordId.trim()}
            onClick={() => {
              const recordId = manualRecordId.trim();
              if (z.uuid().safeParse(recordId).success) runNow.mutate(recordId);
              else {
                runNow.reset();
                setRecordIdInvalid(true);
              }
            }}
            startIcon={<PlayIcon />}
            variant="outlined"
          >
            {t('workflows.runNow')}
          </Button>
        </Stack>
      )}
      {current.enabled_version !== null && (
        <Button
          color="warning"
          disabled={disable.isPending}
          onClick={() => disable.mutate()}
          startIcon={<PowerOffIcon />}
        >
          {t('workflows.disable')}
        </Button>
      )}
    </Stack>
  );
};
