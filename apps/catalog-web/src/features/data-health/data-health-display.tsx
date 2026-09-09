import { Alert } from '@mui/material';

export const SectionError = ({ error }: { error: Error | null }) =>
  error ? <Alert severity="error">{error.message}</Alert> : null;
