import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
} from '@mui/material';
import { useId } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { revokeToken, type PersonalToken } from './api';
import { profileQueryKeys } from './queryKeys';

export const RevokeTokenDialog = ({
  onClose,
  token,
}: {
  onClose: () => void;
  token?: PersonalToken;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const titleId = useId();
  const revoke = useMutation({
    mutationFn: revokeToken,
    onSuccess: async () => {
      await client.invalidateQueries({ queryKey: profileQueryKeys.tokens() });
      onClose();
    },
  });
  const close = () => {
    if (revoke.isPending) return;
    revoke.reset();
    onClose();
  };

  return (
    <Dialog aria-labelledby={titleId} onClose={close} open={Boolean(token)}>
      <DialogTitle id={titleId}>{t('profile.revokeTokenTitle')}</DialogTitle>
      <DialogContent>
        {revoke.isError && (
          <Alert severity="error" sx={{ mb: 4 }}>
            {t('profile.revokeFailed', { message: revoke.error.message })}
          </Alert>
        )}
        <DialogContentText>
          <Trans
            components={{ strong: <strong /> }}
            i18nKey="profile.revokeTokenWarning"
            t={t}
            values={{ label: token?.label ?? '' }}
          />
        </DialogContentText>
      </DialogContent>
      <DialogActions>
        <Button disabled={revoke.isPending} onClick={close}>
          {t('common.cancel')}
        </Button>
        <Button
          color="error"
          disabled={revoke.isPending}
          onClick={() => token && revoke.mutate(token.id)}
          variant="contained"
        >
          {t('profile.revokeToken')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
