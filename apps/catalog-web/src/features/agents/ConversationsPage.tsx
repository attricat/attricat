import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  Alert,
  Button,
  List,
  ListItem,
  ListItemButton,
  ListItemText,
  Paper,
  Typography,
} from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { listConversations } from './api';
import { useTranslation } from 'react-i18next';
import { agentQueryKeys } from './query-keys';

export const ConversationsPage = () => {
  const { i18n, t } = useTranslation();
  const conversations = useQuery({
    queryKey: agentQueryKeys.conversations(),
    queryFn: listConversations,
  });
  return (
    <PageContainer>
      <PageHeader
        actions={
          <Button component={Link} to="/agents/new" variant="contained">
            {t('agents.newConversation')}
          </Button>
        }
        description={t('agents.conversationDescription')}
        title={t('agents.agentConversations')}
      />
      {conversations.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {conversations.error.message}
        </Alert>
      )}
      <Paper sx={{ mt: 3 }}>
        <List disablePadding>
          {conversations.data?.map((conversation) => (
            <ListItem disablePadding divider key={conversation.id}>
              <Link
                params={{ conversationId: conversation.id }}
                to="/agents/$conversationId"
              >
                <ListItemButton>
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
                </ListItemButton>
              </Link>
            </ListItem>
          ))}
          {conversations.isPending && (
            <ListItem>
              <Typography>{t('agents.loadingConversations')}</Typography>
            </ListItem>
          )}
          {conversations.data?.length === 0 && (
            <ListItem>
              <ListItemText
                primary={t('agents.noConversations')}
                secondary={t('agents.noConversationsDescription')}
              />
            </ListItem>
          )}
        </List>
      </Paper>
    </PageContainer>
  );
};
