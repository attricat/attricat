import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Button, Paper, Stack, TextField } from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { createConversation } from './api';
import { agentQueryKeys } from './query-keys';

export const NewConversationPage = () => {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [title, setTitle] = useState('');
  const create = useMutation({
    mutationFn: () => createConversation(title),
    onSuccess: (conversation) => {
      void queryClient.invalidateQueries({
        queryKey: agentQueryKeys.conversations(),
      });
      void navigate({
        to: '/agents/$conversationId',
        params: { conversationId: conversation.id },
      });
    },
  });
  return (
    <PageContainer maxWidth="sm">
      <PageHeader
        description="Create a focused thread for a task or scheduled agent work."
        title="New agent conversation"
      />
      <Paper
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          if (title.trim()) create.mutate();
        }}
        sx={{ mt: 3, p: 2 }}
      >
        <Stack spacing={2}>
          <TextField
            autoFocus
            fullWidth
            label="Conversation title"
            onChange={(event) => setTitle(event.target.value)}
            placeholder="e.g. Clean up inactive suppliers"
            value={title}
          />
          <Button
            disabled={!title.trim() || create.isPending}
            type="submit"
            variant="contained"
          >
            Create conversation
          </Button>
          {create.isError && (
            <Alert severity="error">{create.error.message}</Alert>
          )}
        </Stack>
      </Paper>
    </PageContainer>
  );
};
