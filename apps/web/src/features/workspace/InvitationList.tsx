import { Button, List, ListItem, ListItemText, Paper } from '@mui/material';
import { useMutation } from '@tanstack/react-query';
import { Trans, useTranslation } from 'react-i18next';
import { Timestamp } from '../../time/Timestamp';
import { revokeInvitation } from './api';
import type { Invitation } from './schemas';
import { Trash2Icon } from 'lucide-react';

export const InvitationList = ({
  invitations,
  onChanged,
  onError,
}: {
  invitations?: Invitation[];
  onChanged: () => unknown;
  onError: (error?: Error) => void;
}) => {
  const { t } = useTranslation();
  const revoke = useMutation({
    mutationFn: revokeInvitation,
    onMutate: () => onError(undefined),
    onSuccess: onChanged,
    onError: (reason) => onError(reason),
  });
  return (
    <Paper>
      <List>
        {invitations?.map((item) => (
          <ListItem
            divider
            key={item.id}
            secondaryAction={
              !item.revoked_at &&
              !item.accepted_at && (
                <Button
                  color="error"
                  disabled={revoke.isPending}
                  onClick={() => revoke.mutate(item.id)}
                  startIcon={<Trash2Icon />}
                >
                  {t('workspace.revokeInvitation')}
                </Button>
              )
            }
          >
            <ListItemText
              primary={item.invitee_email}
              secondary={
                <>
                  {item.role_code} ·{' '}
                  <Trans
                    components={{
                      timestamp: <Timestamp value={item.expires_at} />,
                    }}
                    i18nKey="workspace.expires"
                    t={t}
                  />
                </>
              }
            />
          </ListItem>
        ))}
      </List>
    </Paper>
  );
};
