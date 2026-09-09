import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  List,
  ListItem,
  ListItemText,
  Paper,
} from '@mui/material';
import { Link } from '@tanstack/react-router';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import i18n from '../../i18n';
import { listTokens, revokeToken } from './api';
import { profileQueryKeys } from './query-keys';

const formatTime = (value: string | null) =>
  value
    ? new Intl.DateTimeFormat(i18n.language, {
        dateStyle: 'medium',
        timeStyle: 'medium',
      }).format(new Date(value))
    : i18n.t('profile.never');

export const PersonalTokens = ({ canManage }: { canManage: boolean }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [error, setError] = useState<string>();
  const tokens = useQuery({
    enabled: canManage,
    queryKey: profileQueryKeys.tokens(),
    queryFn: listTokens,
  });
  const refresh = () =>
    client.invalidateQueries({ queryKey: profileQueryKeys.tokens() });

  if (!canManage) {
    return <Alert severity="info">{t('profile.tokenUnavailable')}</Alert>;
  }

  return (
    <>
      {error && <Alert severity="error">{error}</Alert>}
      {tokens.isError && <Alert severity="error">{tokens.error.message}</Alert>}
      <Paper>
        <List aria-label={t('profile.tokenList')}>
          {tokens.data?.map((token) => (
            <ListItem
              divider
              key={token.id}
              secondaryAction={
                !token.revoked_at && (
                  <Button
                    color="error"
                    onClick={() =>
                      revokeToken(token.id)
                        .then(refresh)
                        .catch((reason) => setError(reason.message))
                    }
                  >
                    {t('profile.revoke')}
                  </Button>
                )
              }
            >
              <ListItemText
                primary={token.label}
                secondary={t('profile.tokenDetails', {
                  permissions: token.permissions.join(', '),
                  created: formatTime(token.created_at),
                  lastUsed: formatTime(token.last_used_at),
                  expires: formatTime(token.expires_at),
                  revoked: token.revoked_at
                    ? formatTime(token.revoked_at)
                    : t('profile.no'),
                })}
              />
            </ListItem>
          ))}
          {tokens.data?.length === 0 && (
            <ListItem>
              <ListItemText primary={t('profile.noTokens')} />
            </ListItem>
          )}
        </List>
      </Paper>
      <Button
        component={Link}
        to="/profile/personal-access-tokens"
        variant="contained"
      >
        {t('profile.createToken')}
      </Button>
    </>
  );
};
