import { Typography } from '@mui/material';
import { YamlEditor } from '../../components/YamlEditor';
import { editorHeight } from './constants';
import { SectionPaper } from '../../components/SectionPaper';

/** A read-only view of one workflow revision's YAML definition. */
export const WorkflowSourcePanel = ({
  definition,
  title,
}: {
  definition: string;
  title: string;
}) => (
  <SectionPaper>
    <Typography component="h2" variant="h6">
      {title}
    </Typography>
    <YamlEditor
      ariaLabel={title}
      height={editorHeight}
      marginTop={1}
      onChange={undefined}
      readOnly
      value={definition}
    />
  </SectionPaper>
);
