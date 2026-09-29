import { Button, List, ListItem, ListItemText, Paper } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { revokeInvitation } from './api';

type Invitation = {
  accepted_at?: string | null;
  expires_at: string;
  id: string;
  invitee_email: string;
  revoked_at?: string | null;
  role_code: string;
};

export const InvitationList = ({
  invitations,
  onChanged,
  onError,
}: {
  invitations?: Invitation[];
  onChanged: () => unknown;
  onError: (message: string) => void;
}) => {
  const { t } = useTranslation();
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
                  onClick={() =>
                    revokeInvitation(item.id)
                      .then(onChanged)
                      .catch((e: Error) => onError(e.message))
                  }
                >
                  {t('workspace.revokeInvitation')}
                </Button>
              )
            }
          >
            <ListItemText
              primary={item.invitee_email}
              secondary={`${item.role_code} · ${t('workspace.expires', { date: new Date(item.expires_at).toLocaleString() })}`}
            />
          </ListItem>
        ))}
      </List>
    </Paper>
  );
};
