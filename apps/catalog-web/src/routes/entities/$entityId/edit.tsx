import { createFileRoute, redirect } from '@tanstack/react-router';

// Fields are edited in place on the entity page; keep old links working.
export const Route = createFileRoute('/entities/$entityId/edit')({
  beforeLoad: ({ params }) => {
    throw redirect({
      params: { entityId: params.entityId },
      replace: true,
      to: '/entities/$entityId',
    });
  },
});
