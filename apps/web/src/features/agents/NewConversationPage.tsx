import { useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { useNavigate } from '@tanstack/react-router';
import { Box } from '@mui/material';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { createConversation } from './api';
import {
  agentRoutes,
  newConversationMaxWidth,
  newConversationMinRows,
} from './constants';
import { ConversationComposer } from './ConversationComposer';
import { conversationTitleFromFirstMessage } from './conversationTitle';
import { agentQueryKeys } from './queryKeys';
import { AgentIcon } from '../../components/systemIcons';
import { useInitialFocus } from '../../components/initialFocus';

export const NewConversationPage = () => {
  const { t } = useTranslation();
  const formRef = useInitialFocus<HTMLFormElement>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  return (
    <PageContainer>
      <Box sx={{ maxWidth: newConversationMaxWidth, mx: 'auto' }}>
        <PageHeader
          description={t('agents.newConversationDescription')}
          icon={AgentIcon}
          title={t('agents.newAgentConversation')}
          titleVariant="h3"
        />
        <ConversationComposer
          conversationId={null}
          createConversation={async (content) =>
            (
              await createConversation(
                conversationTitleFromFirstMessage(content),
              )
            ).id
          }
          formRef={formRef}
          minRows={newConversationMinRows}
          onSent={async (conversationId) => {
            await queryClient.invalidateQueries({
              queryKey: agentQueryKeys.all(),
            });
            await navigate({
              to: agentRoutes.detail,
              params: { conversationId },
            });
          }}
          sticky={false}
        />
      </Box>
    </PageContainer>
  );
};
