import { Outlet, createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/records/$recordId')({
  component: Outlet,
});
