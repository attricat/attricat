import { createFileRoute } from '@tanstack/react-router';
import { CreateContextPage } from '../../../features/contexts/CreateContextPage';
import { RequireCapability } from '../../../features/auth/RequireCapability';
import { capabilities } from '../../../features/auth/capabilities';

const NewContextRoute = () => (
  <RequireCapability capability={capabilities.contextsWrite}>
    <CreateContextPage />
  </RequireCapability>
);

export const Route = createFileRoute('/manage/contexts/new')({
  component: NewContextRoute,
});
