import { createFileRoute } from '@tanstack/react-router';
import { CreateContextPage } from '../../../features/contexts/CreateContextPage';
import { RequireCapability } from '../../../features/auth/RequireCapability';

const NewContextRoute = () => (
  <RequireCapability capability="contexts_write">
    <CreateContextPage />
  </RequireCapability>
);

export const Route = createFileRoute('/manage/contexts/new')({
  component: NewContextRoute,
});
