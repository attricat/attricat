import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Box,
  Button,
  Drawer,
  IconButton,
  Tooltip,
  Typography,
} from '@mui/material';
import { PencilIcon, RotateCcwIcon, XIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useState } from 'react';
import { createConversation, listConversations } from '../../agents/api';
import { agentQueryKeys } from '../../agents/queryKeys';
import { isTitlePending } from '../../agents/titlePolling';
import { RenameConversationDialog } from '../../agents/RenameConversationDialog';
import {
  ConversationPanel,
  type DraftContext,
} from '../../agents/ConversationPanel';
import {
  CONVERSATION_RECORD_ID_PREFIX_LENGTH,
  CONVERSATION_TITLE_POLL_INTERVAL,
  RECORD_DRAWER_WIDTH,
} from '../constants';

export const RecordAgentDrawer = ({
  recordId,
  contextId,
  offset,
  onClose,
  open,
  draft,
}: {
  recordId: string;
  contextId?: string;
  /** Right edge of the drawer, to open it beside another panel. */
  offset?: string;
  onClose: () => void;
  open: boolean;
  draft?: DraftContext;
}) => {
  const { t } = useTranslation();
  const [renameOpen, setRenameOpen] = useState(false);
  const client = useQueryClient();
  const conversations = useQuery({
    queryKey: agentQueryKeys.conversations(),
    queryFn: listConversations,
    enabled: open,
    refetchInterval: (query) =>
      query.state.data?.some(isTitlePending)
        ? CONVERSATION_TITLE_POLL_INTERVAL
        : false,
  });
  const existing = conversations.data?.find(
    (item) =>
      item.record_id === recordId && item.context_id === (contextId ?? null),
  );
  const create = useMutation({
    mutationFn: () =>
      createConversation(
        t('records.recordConversationTitle', {
          recordId: recordId.slice(0, CONVERSATION_RECORD_ID_PREFIX_LENGTH),
        }),
        {
          record_id: recordId,
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
  const closePanel = () => {
    setRenameOpen(false);
    onClose();
  };
  return (
    <Drawer
      anchor="right"
      onClose={closePanel}
      open={open}
      slotProps={{ paper: { sx: { right: offset } } }}
      variant="persistent"
    >
      <Box
        sx={{
          display: 'flex',
          flexDirection: 'column',
          height: '100%',
          p: 2,
          width: { xs: '100vw', sm: RECORD_DRAWER_WIDTH },
        }}
      >
        <Box sx={{ alignItems: 'center', display: 'flex' }}>
          <Typography
            sx={{
              flexGrow: 1,
              minWidth: 0,
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}
            variant="h6"
          >
            {existing?.title ??
              create.data?.title ??
              t('records.askAboutRecord')}
          </Typography>
          {conversationId && (
            <Tooltip title={t('agents.renameConversation')}>
              <IconButton
                aria-label={t('agents.renameConversation')}
                onClick={() => setRenameOpen(true)}
              >
                <PencilIcon />
              </IconButton>
            </Tooltip>
          )}
          <IconButton aria-label={t('common.close')} onClick={closePanel}>
            <XIcon />
          </IconButton>
        </Box>
        <Typography
          color="text.secondary"
          variant="caption"
          sx={{ overflowWrap: 'anywhere' }}
        >
          {recordId}
          {contextId ? ` · ${contextId}` : ''}
        </Typography>
        {renameOpen && conversationId && (
          <RenameConversationDialog
            conversationId={conversationId}
            initialTitle={existing?.title ?? create.data?.title ?? ''}
            onClose={() => setRenameOpen(false)}
          />
        )}
        {conversations.error && (
          <Alert
            severity="error"
            action={
              <Button
                onClick={() => void conversations.refetch()}
                startIcon={<RotateCcwIcon />}
              >
                {t('errors.retry')}
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
              {t('records.startRecordConversation')}
            </Button>
          </Box>
        )}
      </Box>
    </Drawer>
  );
};
