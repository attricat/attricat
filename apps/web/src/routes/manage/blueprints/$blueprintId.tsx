import { Outlet, createFileRoute } from '@tanstack/react-router';

export const Route = createFileRoute('/manage/blueprints/$blueprintId')({
  component: Outlet,
});
