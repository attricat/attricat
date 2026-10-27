import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { List, ListItem, ListItemText, Paper, Typography } from '@mui/material';
import { PlusIcon } from 'lucide-react';
import { JsonInline } from '../../components/JsonValue';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { EmptyState } from '../../components/EmptyState';
import { ContextIcon } from '../../components/systemIcons';
import { useCan } from '../auth/useCan';
import { capabilities } from '../auth/capabilities';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { contextListOptions } from './queryOptions';
import { RouterButton } from '../../components/RouterLink';

export const ContextsPage = () => {
  const { t } = useTranslation();
  const canCreate = useCan(capabilities.contextsWrite);
  const contexts = useQuery(contextListOptions());
  const contextCodes = new Map(
    contexts.data?.map((context) => [context.id, context.code]) ?? [],
  );
  return (
    <PageContainer>
      <PageHeader
        actions={
          canCreate && (
            <RouterButton
              startIcon={<PlusIcon />}
              to="/manage/contexts/new"
              variant="contained"
            >
              {t('contexts.createContext')}
            </RouterButton>
          )
        }
        icon={ContextIcon}
        title={t('contexts.contexts')}
      />
      {contexts.isPending && (
        <Typography sx={{ mt: 3 }}>{t('contexts.loadingContexts')}</Typography>
      )}
      <ApiErrorAlert error={contexts.error} sx={{ mt: 3 }} />
      {contexts.data && (
        <Paper sx={{ mt: 3 }}>
          <List disablePadding>
            {contexts.data.map((context) => (
              <ListItem divider key={context.id}>
                <ListItemText
                  primary={context.code}
                  secondary={
                    <>
                      {context.parent_id
                        ? t('contexts.parent', {
                            parent:
                              contextCodes.get(context.parent_id) ??
                              t('contexts.unknown'),
                          })
                        : t('contexts.root')}
                      {' · '}
                      <JsonInline value={context.data} />
                    </>
                  }
                />
              </ListItem>
            ))}
            {!contexts.data.length && (
              <ListItem sx={{ display: 'block' }}>
                <EmptyState
                  icon={ContextIcon}
                  title={t('contexts.noContexts')}
                />
              </ListItem>
            )}
          </List>
        </Paper>
      )}
    </PageContainer>
  );
};
