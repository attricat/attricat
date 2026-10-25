import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Trans, useTranslation } from 'react-i18next';
import { revokeToken, type PersonalToken } from './api';
import { profileQueryKeys } from './queryKeys';
import { ConfirmDialog } from '../../components/ConfirmDialog';

export const RevokeTokenDialog = ({
  onClose,
  token,
}: {
  onClose: () => void;
  token?: PersonalToken;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const revoke = useMutation({
    mutationFn: revokeToken,
    onSuccess: async () => {
      await client.invalidateQueries({ queryKey: profileQueryKeys.tokens() });
      onClose();
    },
  });
  const close = () => {
    revoke.reset();
    onClose();
  };

  return (
    <ConfirmDialog
      confirmLabel={t('profile.revokeToken')}
      destructive
      error={revoke.error}
      isPending={revoke.isPending}
      onClose={close}
      onConfirm={() => token && revoke.mutate(token.id)}
      open={Boolean(token)}
      title={t('profile.revokeTokenTitle')}
    >
      <Trans
        components={{ strong: <strong /> }}
        i18nKey="profile.revokeTokenWarning"
        t={t}
        values={{ label: token?.label ?? '' }}
      />
    </ConfirmDialog>
  );
};
