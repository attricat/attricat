import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  List,
  ListItem,
  ListItemText,
  Paper,
} from '@mui/material';
import { useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { Timestamp } from '../../time/Timestamp';
import { listTokens, revokeToken } from './api';
import { profileQueryKeys } from './queryKeys';

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
                secondary={
                  <Trans
                    components={{
                      created: (
                        <Timestamp
                          style="dateTimeSeconds"
                          value={token.created_at}
                        />
                      ),
                      lastused: (
                        <Timestamp
                          fallback={t('profile.never')}
                          style="dateTimeSeconds"
                          value={token.last_used_at}
                        />
                      ),
                      expires: (
                        <Timestamp
                          fallback={t('profile.never')}
                          style="dateTimeSeconds"
                          value={token.expires_at}
                        />
                      ),
                      revoked: (
                        <Timestamp
                          fallback={t('profile.no')}
                          style="dateTimeSeconds"
                          value={token.revoked_at}
                        />
                      ),
                    }}
                    i18nKey="profile.tokenDetails"
                    t={t}
                    values={{ permissions: token.permissions.join(', ') }}
                  />
                }
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
    </>
  );
};
