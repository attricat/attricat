import { Box, MenuItem, Paper, TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { compareSelectMinWidth } from './constants';
import type { Workflow } from './schemas';
import { WorkflowSourcePanel } from './WorkflowSourcePanel';

export const WorkflowComparePanel = ({
  compared,
  current,
  onSelectVersion,
  revisions,
}: {
  compared: Workflow | undefined;
  current: Workflow;
  onSelectVersion: (version: number) => void;
  revisions: Workflow[];
}) => {
  const { t } = useTranslation();
  return (
    <Paper component="section" sx={{ mt: 3, p: 2 }}>
      <TextField
        label={t('workflows.compareRevision')}
        onChange={(event) => onSelectVersion(Number(event.target.value))}
        select
        sx={{ minWidth: compareSelectMinWidth }}
        value={compared?.version ?? ''}
      >
        {revisions.map((revision) => (
          <MenuItem key={revision.version} value={revision.version}>
            v{revision.version} ({t(`workflows.statuses.${revision.status}`)})
          </MenuItem>
        ))}
      </TextField>
      <Box
        sx={{
          display: 'grid',
          gap: 2,
          gridTemplateColumns: { md: '1fr 1fr' },
          mt: 2,
        }}
      >
        <WorkflowSourcePanel
          definition={compared?.definition ?? ''}
          title={t('workflows.compareRevision')}
        />
        <WorkflowSourcePanel
          definition={current.definition}
          title={t('workflows.currentRevision')}
        />
      </Box>
    </Paper>
  );
};
