import { Button, List, ListItem, ListItemText, Paper } from '@mui/material';
import { useMutation } from '@tanstack/react-query';
import { Trans, useTranslation } from 'react-i18next';
import { Timestamp } from '../../time/Timestamp';
import { revokeInvitation } from './api';
import type { Invitation } from './schemas';

// Accepted and revoked invitations show when that happened; open ones show
// when they expire, or expired.
const invitationStatus = (item: Invitation) => {
  if (item.accepted_at)
    return { key: 'workspace.acceptedOn', at: item.accepted_at, open: false };
  if (item.revoked_at)
    return { key: 'workspace.revokedOn', at: item.revoked_at, open: false };
  if (Date.parse(item.expires_at) <= Date.now())
    return { key: 'workspace.expired', at: item.expires_at, open: false };
  return { key: 'workspace.expires', at: item.expires_at, open: true };
};
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
        {invitations?.map((item) => {
          const status = invitationStatus(item);
          return (
            <ListItem
              divider
              key={item.id}
              secondaryAction={
                status.open && (
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
                        timestamp: <Timestamp value={status.at} />,
                      }}
                      i18nKey={status.key}
                      t={t}
                    />
                  </>
                }
              />
            </ListItem>
          );
        })}
      </List>
    </Paper>
  );
};
