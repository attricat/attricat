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
import { agentQueryKeys } from './query-keys';

export const ConversationsPage = () => {
  const conversations = useQuery({
    queryKey: agentQueryKeys.conversations(),
    queryFn: listConversations,
  });
  return (
    <PageContainer>
      <PageHeader
        actions={
          <Button component={Link} to="/agents/new" variant="contained">
            New conversation
          </Button>
        }
        description="Ask the Catalog agent to inspect and change your workspace."
        title="Agent conversations"
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
                    primary={conversation.title || 'Untitled conversation'}
                    secondary={`Updated ${new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(conversation.updated_at))}`}
                  />
                </ListItemButton>
              </Link>
            </ListItem>
          ))}
          {conversations.isPending && (
            <ListItem>
              <Typography>Loading conversations...</Typography>
            </ListItem>
          )}
          {conversations.data?.length === 0 && (
            <ListItem>
              <ListItemText
                primary="No conversations yet."
                secondary="Start a thread to work with the agent."
              />
            </ListItem>
          )}
        </List>
      </Paper>
    </PageContainer>
  );
};
