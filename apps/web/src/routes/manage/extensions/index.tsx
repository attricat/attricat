import { createFileRoute, redirect } from '@tanstack/react-router';

export const Route = createFileRoute('/manage/extensions/')({
  beforeLoad: () => {
    throw redirect({ to: '/manage/extensions/marketplace' });
  },
});
