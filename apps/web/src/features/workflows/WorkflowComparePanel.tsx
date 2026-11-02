import { Box, MenuItem, TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { YamlDiffEditor } from '../../components/YamlDiffEditor';
import { compareSelectMinWidth } from './constants';
import type { Workflow } from './schemas';
import { SectionPaper } from '../../components/SectionPaper';

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
    <SectionPaper>
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
        <YamlDiffEditor
          modified={current.definition}
          modifiedTitle={t('workflows.currentRevision')}
          original={compared?.definition ?? ''}
          originalTitle={
            compared ? revisionLabel(compared) : t('workflows.compareRevision')
          }
        />
      </Box>
    </SectionPaper>
  );
};
