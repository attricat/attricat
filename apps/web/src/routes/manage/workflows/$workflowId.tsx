import { Outlet, createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/manage/workflows/$workflowId')({
  component: Outlet,
});
