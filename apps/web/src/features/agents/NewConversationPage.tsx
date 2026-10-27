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
import { AgentIcon } from '../../components/systemIcons';
import { useInitialFocus } from '../../components/initialFocus';

export const NewConversationPage = () => {
  const { t } = useTranslation();
  const formRef = useInitialFocus<HTMLFormElement>();
  const navigate = useNavigate();

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
          onSent={(conversationId) =>
            void navigate({
              to: agentRoutes.detail,
              params: { conversationId },
            })
          }
          sticky={false}
        />
      </Box>
    </PageContainer>
  );
};
