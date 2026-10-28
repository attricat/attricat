import { Paper, Typography } from '@mui/material';
import { TomlEditor } from '../../components/TomlEditor';
import { editorHeight } from './constants';

/** A read-only view of one workflow revision's TOML definition. */
export const WorkflowSourcePanel = ({
  definition,
  title,
}: {
  definition: string;
  title: string;
}) => (
  <Paper component="section" sx={{ mt: 3, p: 2 }}>
    <Typography component="h2" variant="h6">
      {title}
    </Typography>
    <TomlEditor
      ariaLabel={title}
      height={editorHeight}
      marginTop={1}
      onChange={undefined}
      readOnly
      value={definition}
    />
  </Paper>
);
