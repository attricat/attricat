import { createFileRoute } from '@tanstack/react-router';
import { UserProfilePage } from '../../features/users/UserProfilePage';

const UserProfileRoute = () => {
  const { userId } = Route.useParams();
  return <UserProfilePage userId={userId} />;
};

export const Route = createFileRoute('/users/$userId')({
  component: UserProfileRoute,
});
