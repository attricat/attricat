import { useInfiniteQuery } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import {
  Box,
  Button,
  FormControlLabel,
  List,
  ListItem,
  ListItemText,
  Paper,
  Switch,
  TextField,
} from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { NextPageButton } from '../../components/LoadMoreButton';
import {
  RouterButton,
  RouterListItemButton,
} from '../../components/RouterLink';
import { Trans, useTranslation } from 'react-i18next';
import { PlusIcon, SearchIcon } from 'lucide-react';
import { Timestamp } from '../../time/Timestamp';
import {
  agentRoutes,
  conversationScopes,
  maximumSearchLength,
} from './constants';
import { conversationSearchOptions } from './queryOptions';
import { EmptyState } from '../../components/EmptyState';
import { AgentIcon } from '../../components/systemIcons';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { LoadingState } from '../../components/LoadingState';

export const ConversationsPage = ({
  search,
}: {
  search: { q?: string; scope?: typeof conversationScopes.all };
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate({ from: '/agents/' });
  const [draftQuery, setDraftQuery] = useState(search.q ?? '');
  const submitSearch = () =>
    void navigate({
      to: agentRoutes.list,
      search: { q: draftQuery.trim() || undefined, scope: search.scope },
    });
  const query = search.q ?? '';
  const scope = search.scope ?? conversationScopes.unattached;
  const conversations = useInfiniteQuery(
    conversationSearchOptions(query, { scope }),
  );
  const items = conversations.data?.pages.flatMap((page) => page.items) ?? [];
  return (
    <PageContainer>
      <PageHeader
        actions={
          <RouterButton
            startIcon={<PlusIcon />}
            to={agentRoutes.create}
            variant="contained"
          >
            {t('agents.newConversation')}
          </RouterButton>
        }
        description={t('agents.conversationDescription')}
        icon={AgentIcon}
        title={t('agents.agentConversations')}
      />
      <Box
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          submitSearch();
        }}
        sx={{ alignItems: 'center', display: 'flex', gap: 1, mt: 3 }}
      >
        <TextField
          fullWidth
          label={t('agents.searchConversations')}
          onChange={(event) => setDraftQuery(event.target.value)}
          slotProps={{ htmlInput: { maxLength: maximumSearchLength } }}
          value={draftQuery}
        />
        <Button startIcon={<SearchIcon />} type="submit" variant="outlined">
          {t('agents.search')}
        </Button>
      </Box>
      <FormControlLabel
        control={
          <Switch
            checked={scope === conversationScopes.all}
            onChange={(event) =>
              void navigate({
                to: agentRoutes.list,
                search: {
                  q: search.q,
                  scope: event.target.checked
                    ? conversationScopes.all
                    : undefined,
                },
              })
            }
          />
        }
        label={t('agents.showRecordConversations')}
        sx={{ mt: 1 }}
      />
      <ApiErrorAlert error={conversations.error} sx={{ mt: 3 }} />
      <Paper sx={{ mt: 3 }}>
        <List disablePadding>
          {items.map((conversation) => (
            <ListItem
              disablePadding
              divider
              key={conversation.id}
              secondaryAction={
                conversation.record_id && (
                  <RouterButton
                    params={{ recordId: conversation.record_id }}
                    size="small"
                    to={agentRoutes.record}
                  >
                    {t('agents.openRecord')}
                  </RouterButton>
                )
              }
            >
              <RouterListItemButton
                params={{ conversationId: conversation.id }}
                sx={{ width: '100%' }}
                to={agentRoutes.detail}
              >
                <ListItemText
                  primary={
                    conversation.title || t('agents.untitledConversation')
                  }
                  secondary={
                    <Trans
                      components={{
                        timestamp: (
                          <Timestamp
                            focusable={false}
                            value={conversation.updated_at}
                          />
                        ),
                      }}
                      i18nKey="agents.updatedAt"
                      t={t}
                    />
                  }
                />
              </RouterListItemButton>
            </ListItem>
          ))}
          {conversations.isPending && (
            <ListItem>
              <LoadingState>{t('agents.loadingConversations')}</LoadingState>
            </ListItem>
          )}
          {!conversations.isPending &&
            !conversations.isError &&
            items.length === 0 &&
            !conversations.hasNextPage && (
              <ListItem sx={{ display: 'block' }}>
                <EmptyState
                  description={t(
                    query
                      ? 'agents.noMatchingConversationsDescription'
                      : 'agents.noConversationsDescription',
                  )}
                  icon={AgentIcon}
                  title={t(
                    query
                      ? 'agents.noMatchingConversations'
                      : 'agents.noConversations',
                  )}
                />
              </ListItem>
            )}
        </List>
      </Paper>
      <NextPageButton query={conversations} sx={{ mt: 2 }} />
    </PageContainer>
  );
};
