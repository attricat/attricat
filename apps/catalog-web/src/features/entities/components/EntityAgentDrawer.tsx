import CloseIcon from '@mui/icons-material/Close';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Box,
  Button,
  Drawer,
  IconButton,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { createConversation, listConversations } from '../../agents/api';
import { agentQueryKeys } from '../../agents/queryKeys';
import {
  ConversationPanel,
  type DraftContext,
} from '../../agents/ConversationPanel';

export const EntityAgentDrawer = ({
  entityId,
  contextId,
  onClose,
  open,
  draft,
}: {
  entityId: string;
  contextId?: string;
  onClose: () => void;
  open: boolean;
  draft?: DraftContext;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const conversations = useQuery({
    queryKey: agentQueryKeys.conversations(),
    queryFn: listConversations,
    enabled: open,
  });
  const existing = conversations.data?.find(
    (item) =>
      item.entity_id === entityId && item.context_id === (contextId ?? null),
  );
  const create = useMutation({
    mutationFn: () =>
      createConversation(
        `${t('entities.askAboutEntity')} · ${entityId.slice(0, 8)}`,
        {
          entity_id: entityId,
          context_id: contextId,
        },
      ),
    onSuccess: () =>
      void client.invalidateQueries({
        queryKey: agentQueryKeys.conversations(),
      }),
  });
  const conversationId =
    existing?.id ?? (create.data && !existing ? create.data.id : undefined);
  return (
    <Drawer anchor="right" onClose={onClose} open={open} variant="persistent">
      <Box
        sx={{
          display: 'flex',
          flexDirection: 'column',
          height: '100%',
          p: 2,
          width: { xs: '100vw', sm: 480 },
        }}
      >
        <Box sx={{ alignItems: 'center', display: 'flex' }}>
          <Typography sx={{ flexGrow: 1 }} variant="h6">
            {t('entities.askAboutEntity')}
          </Typography>
          <IconButton aria-label={t('common.close')} onClick={onClose}>
            <CloseIcon />
          </IconButton>
        </Box>
        <Typography
          color="text.secondary"
          variant="caption"
          sx={{ overflowWrap: 'anywhere' }}
        >
          {entityId}
          {contextId ? ` · ${contextId}` : ''}
        </Typography>
        {conversations.error && (
          <Alert
            severity="error"
            action={
              <Button onClick={() => void conversations.refetch()}>
                {t('common.retry')}
              </Button>
            }
          >
            {conversations.error.message}
          </Alert>
        )}
        {create.error && <Alert severity="error">{create.error.message}</Alert>}
        {open && conversationId ? (
          <ConversationPanel
            key={conversationId}
            conversationId={conversationId}
            draft={draft}
          />
        ) : (
          <Box sx={{ mt: 3 }}>
            <Button
              disabled={create.isPending || !conversations.isSuccess}
              onClick={() => create.mutate()}
              variant="contained"
            >
              {t('entities.startEntityConversation')}
            </Button>
          </Box>
        )}
      </Box>
    </Drawer>
  );
};
