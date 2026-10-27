import { useInfiniteQuery } from '@tanstack/react-query';
import {
  Box,
  Button,
  Chip,
  CircularProgress,
  List,
  ListItem,
  ListItemButton,
  ListItemText,
  Tooltip,
} from '@mui/material';
import { ExternalLinkIcon, PlusIcon } from 'lucide-react';
import { Trans, useTranslation } from 'react-i18next';
import { agentRoutes } from './constants';
import { conversationSearchOptions } from './queryOptions';
import { EmptyState } from '../../components/EmptyState';
import { AgentIcon } from '../../components/systemIcons';
import { NextPageButton } from '../../components/LoadMoreButton';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { RouterIconButton } from '../../components/RouterLink';
import { Timestamp } from '../../time/Timestamp';
import { PrincipalText } from '../principals/PrincipalValue';
import { principalKinds } from '../principals/constants';
import { principalReference } from '../principals/principal';

/**
 * Everyone's conversations about a record, newest first. Selecting one
 * continues it beside the record.
 */
export const RecordConversationsPanel = ({
  recordId,
  contextCodes,
  onOpen,
  onStart,
}: {
  recordId: string;
  /** Codes of the record's contexts by ID, to name a conversation's context. */
  contextCodes: ReadonlyMap<string, string>;
  onOpen: (conversationId: string) => void;
  onStart: () => void;
}) => {
  const { t } = useTranslation();
  const conversations = useInfiniteQuery(
    conversationSearchOptions('', { recordId }),
  );
  const items = conversations.data?.pages.flatMap((page) => page.items) ?? [];
  return (
    <Box>
      <Button onClick={onStart} startIcon={<PlusIcon />} variant="outlined">
        {t('agents.newConversation')}
      </Button>
      {conversations.isPending && (
        <Box sx={{ mt: 2 }}>
          <CircularProgress aria-label={t('agents.loadingConversations')} />
        </Box>
      )}
      <QueryErrorNotice
        error={conversations.error}
        isRetrying={conversations.isFetching}
        onRetry={() => void conversations.refetch()}
      />
      {conversations.isSuccess && items.length === 0 && (
        <EmptyState
          description={t('records.agentConversations.emptyDescription')}
          icon={AgentIcon}
          title={t('records.agentConversations.empty')}
        />
      )}
      {items.length > 0 && (
        <List sx={{ mt: 1 }}>
          {items.map((conversation) => {
            const contextCode =
              conversation.context_id &&
              contextCodes.get(conversation.context_id);
            return (
              <ListItem
                disablePadding
                divider
                key={conversation.id}
                secondaryAction={
                  <Tooltip title={t('records.agentConversations.openPage')}>
                    <RouterIconButton
                      aria-label={t('records.agentConversations.openPage')}
                      edge="end"
                      params={{ conversationId: conversation.id }}
                      to={agentRoutes.detail}
                    >
                      <ExternalLinkIcon />
                    </RouterIconButton>
                  </Tooltip>
                }
              >
                <ListItemButton onClick={() => onOpen(conversation.id)}>
                  <ListItemText
                    primary={
                      conversation.title || t('agents.untitledConversation')
                    }
                    secondary={
                      <>
                        {conversation.created_by_user_id && (
                          <>
                            <PrincipalText
                              value={principalReference(
                                principalKinds.user,
                                conversation.created_by_user_id,
                              )}
                            />
                            {' · '}
                          </>
                        )}
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
                        {contextCode && (
                          <Chip
                            component="span"
                            label={contextCode}
                            size="small"
                            sx={{ ml: 1 }}
                            variant="outlined"
                          />
                        )}
                      </>
                    }
                  />
                </ListItemButton>
              </ListItem>
            );
          })}
        </List>
      )}
      <NextPageButton query={conversations} sx={{ mt: 2 }} />
    </Box>
  );
};
