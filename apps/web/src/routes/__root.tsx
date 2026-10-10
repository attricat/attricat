import { createRootRoute } from '@tanstack/react-router';
import { AppLayout } from '../components/AppLayout';
import {
  NotFoundScreen,
  RouterErrorScreen,
} from '../components/RouterRecoveryScreens';

export const Route = createRootRoute({
  component: AppLayout,
  errorComponent: ({ reset }) => <RouterErrorScreen onRetry={reset} />,
  notFoundComponent: NotFoundScreen,
});
