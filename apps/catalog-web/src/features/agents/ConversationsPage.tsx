import { useInfiniteQuery } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import {
  Alert,
  Box,
  Button,
  List,
  ListItem,
  ListItemText,
  Paper,
  TextField,
  Typography,
} from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { LoadMoreButton } from '../../components/LoadMoreButton';
import { RouterListItemButton } from '../../components/RouterLink';
import { searchConversations } from './api';
import { useTranslation } from 'react-i18next';
import {
  agentRoutes,
  maximumSearchLength,
  titlePollIntervalMs,
} from './constants';
import { agentQueryKeys } from './queryKeys';
import { isTitlePending } from './titlePolling';

export const ConversationsPage = ({ search }: { search: { q?: string } }) => {
  const { i18n, t } = useTranslation();
  const navigate = useNavigate({ from: '/agents/' });
  const [draftQuery, setDraftQuery] = useState(search.q ?? '');
  const submitSearch = () =>
    void navigate({
      to: agentRoutes.list,
      search: { q: draftQuery.trim() || undefined },
    });
  const query = search.q ?? '';
  const conversations = useInfiniteQuery({
    queryKey: agentQueryKeys.conversationSearch(query),
    queryFn: ({ pageParam }) => searchConversations(query, pageParam),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) => page.next_cursor ?? undefined,
    refetchInterval: (state) =>
      state.state.data?.pages.some((page) => page.items.some(isTitlePending))
        ? titlePollIntervalMs
        : false,
  });
  const items = conversations.data?.pages.flatMap((page) => page.items) ?? [];
  return (
    <PageContainer>
      <PageHeader
        actions={
          <Button component={Link} to={agentRoutes.create} variant="contained">
            {t('agents.newConversation')}
          </Button>
        }
        description={t('agents.conversationDescription')}
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
        <Button type="submit" variant="outlined">
          {t('agents.search')}
        </Button>
      </Box>
      {conversations.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {conversations.error.message}
        </Alert>
      )}
      <Paper sx={{ mt: 3 }}>
        <List disablePadding>
          {items.map((conversation) => (
            <ListItem disablePadding divider key={conversation.id}>
              <RouterListItemButton
                params={{ conversationId: conversation.id }}
                sx={{ width: '100%' }}
                to={agentRoutes.detail}
              >
                <ListItemText
                  primary={
                    conversation.title || t('agents.untitledConversation')
                  }
                  secondary={t('agents.updatedAt', {
                    date: new Intl.DateTimeFormat(i18n.language, {
                      dateStyle: 'medium',
                      timeStyle: 'short',
                    }).format(new Date(conversation.updated_at)),
                  })}
                />
              </RouterListItemButton>
            </ListItem>
          ))}
          {conversations.isPending && (
            <ListItem>
              <Typography>{t('agents.loadingConversations')}</Typography>
            </ListItem>
          )}
          {!conversations.isPending &&
            !conversations.isError &&
            items.length === 0 &&
            !conversations.hasNextPage && (
              <ListItem>
                <ListItemText
                  primary={t(
                    query
                      ? 'agents.noMatchingConversations'
                      : 'agents.noConversations',
                  )}
                  secondary={t(
                    query
                      ? 'agents.noMatchingConversationsDescription'
                      : 'agents.noConversationsDescription',
                  )}
                />
              </ListItem>
            )}
        </List>
      </Paper>
      {conversations.hasNextPage && (
        <Box sx={{ mt: 2 }}>
          <LoadMoreButton
            isLoading={conversations.isFetchingNextPage}
            onLoadMore={() => void conversations.fetchNextPage()}
          />
        </Box>
      )}
    </PageContainer>
  );
};
