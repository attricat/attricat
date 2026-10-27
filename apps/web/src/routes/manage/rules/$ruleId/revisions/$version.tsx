import { Outlet, createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute(
  '/manage/rules/$ruleId/revisions/$version',
)({
  component: Outlet,
});
