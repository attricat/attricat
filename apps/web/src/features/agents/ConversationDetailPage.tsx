import { useQuery } from '@tanstack/react-query';
import { Alert, Box, Button } from '@mui/material';
import { PencilIcon } from 'lucide-react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useResourcePageTitle } from '../../app/useResourcePageTitle';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { conversationOptions } from './queryOptions';
import { conversationMinHeight } from './constants';
import { ConversationPanel } from './ConversationPanel';
import { titlePollInterval } from './titlePolling';
import { RenameConversationDialog } from './RenameConversationDialog';
import { AgentIcon } from '../../components/systemIcons';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';

type ConversationDetailPageProps = { conversationId: string };

export const ConversationDetailPage = ({
  conversationId,
}: ConversationDetailPageProps) => {
  const { t } = useTranslation();
  const [renameOpen, setRenameOpen] = useState(false);
  const conversation = useQuery({
    ...conversationOptions(conversationId),
    refetchInterval: (query) =>
      titlePollInterval(query.state.data && [query.state.data]),
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
                startIcon={<PencilIcon />}
                variant="outlined"
              >
                {t('agents.renameConversation')}
              </Button>
            )
          }
          eyebrow={t('agents.agentConversation')}
          icon={AgentIcon}
          title={conversation.data?.title ?? t('agents.conversation')}
          titleVariant="h3"
        />
        {conversation.data?.read_only && (
          <Alert severity="info" sx={{ mb: 2 }}>
            {t('agents.readOnlyNotice')}
          </Alert>
        )}
        <ApiErrorAlert error={conversation.error} />
        {renameOpen && conversation.data && (
          <RenameConversationDialog
            key={conversationId}
            conversationId={conversationId}
            initialTitle={conversation.data.title}
            onClose={() => setRenameOpen(false)}
          />
        )}
        {/* The panel would repeat the failure of a conversation that cannot
            be read. */}
        {!conversation.isError && (
          <ConversationPanel
            key={conversationId}
            conversationId={conversationId}
          />
        )}
      </Box>
    </PageContainer>
  );
};
