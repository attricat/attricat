import { Alert } from '@mui/material';

export const ErrorNotice = ({ error }: { error: Error | null }) =>
  error ? (
    <Alert severity="error" sx={{ mb: 2 }}>
      {error.message}
    </Alert>
  ) : null;
