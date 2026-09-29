import { useQuery } from '@tanstack/react-query';
import EditOutlinedIcon from '@mui/icons-material/EditOutlined';
import { Alert, Box, Button } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useResourcePageTitle } from '../../app/useResourcePageTitle';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { getConversation } from './api';
import { conversationMinHeight, titlePollIntervalMs } from './constants';
import { agentQueryKeys } from './queryKeys';
import { ConversationPanel } from './ConversationPanel';
import { isTitlePending } from './titlePolling';
import { RenameConversationDialog } from './RenameConversationDialog';

export type ConversationDetailPageProps = { conversationId: string };

export const ConversationDetailPage = ({
  conversationId,
}: ConversationDetailPageProps) => {
  const { t } = useTranslation();
  const [renameOpen, setRenameOpen] = useState(false);
  const conversation = useQuery({
    queryKey: agentQueryKeys.conversation(conversationId),
    queryFn: () => getConversation(conversationId),
    refetchInterval: (query) =>
      query.state.data && isTitlePending(query.state.data)
        ? titlePollIntervalMs
        : false,
  });
  useResourcePageTitle(conversation.data?.title, t('agents.agentConversation'));
  return (
    <PageContainer maxWidth={false}>
      <Box
        sx={{
          display: 'flex',
          flexDirection: 'column',
          minHeight: conversationMinHeight,
          mx: 'auto',
          width: '100%',
        }}
      >
        <PageHeader
          actions={
            conversation.data && (
              <Button
                onClick={() => setRenameOpen(true)}
                startIcon={<EditOutlinedIcon />}
                variant="outlined"
              >
                {t('agents.renameConversation')}
              </Button>
            )
          }
          eyebrow={t('agents.agentConversation')}
          title={conversation.data?.title ?? t('agents.conversation')}
          titleVariant="h3"
        />
        {conversation.error && (
          <Alert severity="error">{conversation.error.message}</Alert>
        )}
        {renameOpen && conversation.data && (
          <RenameConversationDialog
            conversationId={conversationId}
            initialTitle={conversation.data.title}
            onClose={() => setRenameOpen(false)}
          />
        )}
        <ConversationPanel conversationId={conversationId} />
      </Box>
    </PageContainer>
  );
};
