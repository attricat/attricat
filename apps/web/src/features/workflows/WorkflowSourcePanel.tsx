import { Box, Paper, Typography } from '@mui/material';
import { monoFontFamily } from '../../app/theme';

export const WorkflowSourcePanel = ({
  definition,
  title,
}: {
  definition: string;
  title?: string;
}) => (
  <Paper component="section" sx={{ mt: title ? 0 : 3, p: 2 }}>
    <Typography component="h2" variant="h6">
      {title}
    </Typography>
    <Box
      aria-label={title}
      component="pre"
      sx={{
        fontFamily: monoFontFamily,
        m: 0,
        mt: title ? 1 : 0,
        overflow: 'auto',
        whiteSpace: 'pre-wrap',
      }}
    >
      {definition}
    </Box>
  </Paper>
);
