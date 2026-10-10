import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import {
  Alert,
  Button,
  List,
  ListItem,
  ListItemText,
  Paper,
  Typography,
} from '@mui/material';
import { PlusIcon } from 'lucide-react';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { listContexts } from './api';
import { contextQueryKeys } from './queryKeys';
import { EmptyState } from '../../components/EmptyState';
import { ContextIcon } from '../../components/systemIcons';

export const ContextsPage = () => {
  const { t } = useTranslation();
  const contexts = useQuery({
    queryKey: contextQueryKeys.all(),
    queryFn: ({ signal }) => listContexts(signal),
  });
  const contextCodes = new Map(
    contexts.data?.map((context) => [context.id, context.code]) ?? [],
  );
  return (
    <PageContainer>
      <PageHeader
        actions={
          <Button
            component={Link}
            startIcon={<PlusIcon />}
            to="/manage/contexts/new"
            variant="contained"
          >
            {t('contexts.createContext')}
          </Button>
        }
        icon={ContextIcon}
        title={t('contexts.contexts')}
      />
      {contexts.isPending && (
        <Typography sx={{ mt: 3 }}>{t('contexts.loadingContexts')}</Typography>
      )}
      {contexts.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {contexts.error.message}
        </Alert>
      )}
      {contexts.data && (
        <Paper sx={{ mt: 3 }}>
          <List disablePadding>
            {contexts.data.map((context) => (
              <ListItem divider key={context.id}>
                <ListItemText
                  primary={context.code}
                  secondary={[
                    context.parent_id
                      ? t('contexts.parent', {
                          parent:
                            contextCodes.get(context.parent_id) ??
                            t('contexts.unknown'),
                        })
                      : t('contexts.root'),
                    JSON.stringify(context.data),
                  ].join(' · ')}
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
