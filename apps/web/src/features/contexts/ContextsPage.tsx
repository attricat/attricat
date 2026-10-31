import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, List, ListItem, ListItemText, Paper } from '@mui/material';
import { PlusIcon, Trash2Icon } from 'lucide-react';
import { ConfirmDialog } from '../../components/ConfirmDialog';
import { deleteContext, type AttributeContext } from './api';
import { defaultContextCode } from './constants';
import { contextQueryKeys } from './queryKeys';
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
import { LoadingState } from '../../components/LoadingState';

export const ContextsPage = () => {
  const { t } = useTranslation();
  const canCreate = useCan(capabilities.contextsWrite);
  const contexts = useQuery(contextListOptions());
  const client = useQueryClient();
  const [deleting, setDeleting] = useState<{
    context: AttributeContext;
    open: boolean;
  } | null>(null);
  const hideDelete = () =>
    setDeleting((current) => current && { ...current, open: false });
  const remove = useMutation({
    mutationFn: (context: AttributeContext) => deleteContext(context.id),
    onSuccess: () => {
      // Close before refetching: a slow refetch must not hold the dialog.
      hideDelete();
      void client.invalidateQueries({ queryKey: contextQueryKeys.all() });
    },
  });
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
        <LoadingState sx={{ mt: 3 }}>
          {t('contexts.loadingContexts')}
        </LoadingState>
      )}
      <ApiErrorAlert error={contexts.error} sx={{ mt: 3 }} />
      {contexts.data && (
        <Paper sx={{ mt: 3 }}>
          <List disablePadding>
            {contexts.data.map((context) => (
              <ListItem
                divider
                key={context.id}
                secondaryAction={
                  canCreate &&
                  context.code !== defaultContextCode && (
                    <Button
                      aria-label={t('contexts.deleteNamedContext', {
                        context: context.code,
                      })}
                      color="error"
                      onClick={() => {
                        remove.reset();
                        setDeleting({ context, open: true });
                      }}
                      size="small"
                      startIcon={<Trash2Icon />}
                    >
                      {t('contexts.deleteContext')}
                    </Button>
                  )
                }
              >
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
      <ConfirmDialog
        confirmLabel={t('contexts.deleteContext')}
        destructive
        error={remove.error}
        isPending={remove.isPending}
        onClose={() => {
          if (!remove.isPending) hideDelete();
        }}
        onConfirm={() => deleting?.open && remove.mutate(deleting.context)}
        open={Boolean(deleting?.open)}
        slotProps={{ transition: { onExited: () => setDeleting(null) } }}
        title={t('contexts.deleteContext')}
      >
        {t('contexts.deleteContextConfirm', {
          context: deleting?.context.code,
        })}
      </ConfirmDialog>
    </PageContainer>
  );
};
