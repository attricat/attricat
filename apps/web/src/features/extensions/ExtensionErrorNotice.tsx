import { ApiErrorAlert } from '../../components/ApiErrorAlert';

export const ErrorNotice = ({ error }: { error: Error | null }) =>
  error ? <ApiErrorAlert error={error} sx={{ mb: 2 }} /> : null;
