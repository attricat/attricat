import { Paper, type PaperProps } from '@mui/material';

/** The surface of one section of a page, spaced from what comes before it. */
export const SectionPaper = (
  props: Omit<PaperProps<'section'>, 'component' | 'sx'>,
) => <Paper {...props} component="section" sx={{ mt: 3, p: 2.5 }} />;
