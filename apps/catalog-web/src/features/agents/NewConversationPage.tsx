import { useState } from 'react';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Button, Paper, Stack, TextField } from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { createConversation } from './api';
import { agentQueryKeys } from './query-keys';

export const NewConversationPage = () => {
  const { t } = useTranslation();
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
    <PageContainer>
      <PageHeader
        description={t('agents.newConversationDescription')}
        title={t('agents.newAgentConversation')}
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
            label={t('agents.conversationTitle')}
            onChange={(event) => setTitle(event.target.value)}
            placeholder={t('agents.conversationTitlePlaceholder')}
            value={title}
          />
          <Button
            disabled={!title.trim() || create.isPending}
            type="submit"
            variant="contained"
          >
            {t('agents.createConversation')}
          </Button>
          {create.isError && (
            <Alert severity="error">{create.error.message}</Alert>
          )}
        </Stack>
      </Paper>
    </PageContainer>
  );
};
