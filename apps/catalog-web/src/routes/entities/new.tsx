import { createFileRoute } from '@tanstack/react-router';
import { CreateEntityPage } from '../../features/entities/CreateEntityPage';

export const Route = createFileRoute('/entities/new')({
  component: CreateEntityPage,
});
