import { Link } from '@tanstack/react-router';
import { Button, Stack, TextField } from '@mui/material';
import { type ComponentType, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { UseMutationResult } from '@tanstack/react-query';
import {
  PencilIcon,
  PlayIcon,
  PowerIcon,
  PowerOffIcon,
  SendIcon,
} from 'lucide-react';
import { workflowRoutes, workflowStatus } from './constants';
import type { Workflow } from './schemas';

const WorkflowRevisionLink = Link as unknown as ComponentType<{
  params: { version: string; workflowId: string };
  to: typeof workflowRoutes.newRevision;
}>;

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
  const [manualEntityId, setManualEntityId] = useState('');
  const isEnabledRevision = current.enabled_version === current.version;
  return (
    <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap' }}>
      <Button
        component={WorkflowRevisionLink}
        params={{ workflowId, version: String(current.version) }}
        startIcon={<PencilIcon />}
        to={workflowRoutes.newRevision}
        variant="outlined"
      >
        {t('workflows.newRevision')}
      </Button>
      {current.status === workflowStatus.draft && (
        <Button
          disabled={publish.isPending}
          onClick={() => publish.mutate(current.version)}
          startIcon={<SendIcon />}
          variant="contained"
        >
          {t('workflows.publish')}
        </Button>
      )}
      {current.status === workflowStatus.published && !isEnabledRevision && (
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
        <Stack direction="row" spacing={1}>
          <TextField
            disabled={runNow.isPending}
            label={t('workflows.entityId')}
            required
            slotProps={{
              htmlInput: { 'aria-label': t('workflows.manualRunEntityId') },
            }}
            onChange={(event) => setManualEntityId(event.target.value)}
            size="small"
            value={manualEntityId}
          />
          <Button
            disabled={runNow.isPending || !manualEntityId.trim()}
            onClick={() => runNow.mutate(manualEntityId.trim())}
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
