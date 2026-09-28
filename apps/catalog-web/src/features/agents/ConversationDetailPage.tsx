import { useQuery } from '@tanstack/react-query';
import { Alert, Box } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { getConversation } from './api';
import { agentQueryKeys } from './queryKeys';
import { ConversationPanel } from './ConversationPanel';

export type ConversationDetailPageProps = { conversationId: string };

export const ConversationDetailPage = ({
  conversationId,
}: ConversationDetailPageProps) => {
  const { t } = useTranslation();
  const conversation = useQuery({
    queryKey: agentQueryKeys.conversation(conversationId),
    queryFn: () => getConversation(conversationId),
  });
  return (
    <PageContainer maxWidth={false}>
      <Box
        sx={{
          display: 'flex',
          flexDirection: 'column',
          minHeight: { md: 'calc(100dvh - 48px)' },
          mx: 'auto',
          width: '100%',
        }}
      >
        <PageHeader
          eyebrow={t('agents.agentConversation')}
          title={conversation.data?.title ?? t('agents.conversation')}
          titleVariant="h3"
        />
        {conversation.error && (
          <Alert severity="error">{conversation.error.message}</Alert>
        )}
        <ConversationPanel conversationId={conversationId} />
      </Box>
    </PageContainer>
  );
};
