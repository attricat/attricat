import { useTranslation } from 'react-i18next';
import { ConfirmDialog } from '../../components/ConfirmDialog';
import { useActionConfirmStore } from './actionConfirmStore';

/** Mounted once in the application shell, like the action dialog host. */
export const ExtensionActionConfirmHost = () => {
  const { t } = useTranslation();
  const pending = useActionConfirmStore((state) => state.pending);
  const answer = useActionConfirmStore((state) => state.answer);
  if (!pending) return null;
  return (
    <ConfirmDialog
      confirmLabel={t('extensions.actionConfirm')}
      destructive={pending.destructive}
      error={null}
      isPending={false}
      onClose={() => answer(false)}
      onConfirm={() => answer(true)}
      title={pending.title}
    >
      {pending.message}
    </ConfirmDialog>
  );
};
