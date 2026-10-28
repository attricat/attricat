import { Box, MenuItem, Paper, TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { TomlDiffEditor } from '../../components/TomlDiffEditor';
import { compareSelectMinWidth } from './constants';
import type { Workflow } from './schemas';

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
  const revisionLabel = ({ version, status }: Workflow) =>
    `v${version} (${t(`common.revisionStatuses.${status}`)})`;
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
            {revisionLabel(revision)}
          </MenuItem>
        ))}
      </TextField>
      <Box sx={{ mt: 2 }}>
        <TomlDiffEditor
          modified={current.definition}
          modifiedTitle={t('workflows.currentRevision')}
          original={compared?.definition ?? ''}
          originalTitle={
            compared ? revisionLabel(compared) : t('workflows.compareRevision')
          }
        />
      </Box>
    </Paper>
  );
};
