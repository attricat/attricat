import { createFileRoute } from '@tanstack/react-router';
import { ExtensionRoutePage } from '../../../features/extensions/ExtensionOutlet';

const ExtensionRoute = () => {
  const { contributionId, extensionId } = Route.useParams();
  return (
    <ExtensionRoutePage
      contributionId={contributionId}
      extensionId={extensionId}
    />
  );
};

export const Route = createFileRoute(
  '/extensions/$extensionId/$contributionId',
)({
  component: ExtensionRoute,
});
