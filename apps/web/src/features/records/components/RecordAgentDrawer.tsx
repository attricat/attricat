import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Box,
  Button,
  Drawer,
  IconButton,
  Tooltip,
  Typography,
} from '@mui/material';
import { ListIcon, PencilIcon, PlusIcon, XIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useState } from 'react';
import {
  createConversation,
  setConversationAutoApproval,
} from '../../agents/api';
import { AutoApprovalSwitch } from '../../agents/AutoApprovalSwitch';
import { composerSurfaces } from '../../agents/constants';
import { invalidateRecord } from '../invalidateRecord';
import { sessionOptions } from '../../auth/queryOptions';
import { conversationTitleFromFirstMessage } from '../../agents/conversationTitle';
import { conversationOptions } from '../../agents/queryOptions';
import { titlePollInterval } from '../../agents/titlePolling';
import { RenameConversationDialog } from '../../agents/RenameConversationDialog';
import {
  ConversationPanel,
  StartConversationPanel,
  type DraftContext,
} from '../../agents/ConversationPanel';
import { RECORD_DRAWER_WIDTH } from '../constants';
import { ApiErrorAlert } from '../../../components/ApiErrorAlert';

/**
 * Asks the agent about a record: a new conversation, created by its first
 * message, or one of the record's earlier conversations.
 */
export const RecordAgentDrawer = ({
  recordId,
  contextId,
  conversationId,
  onConversationChange,
  onShowAll,
  offset,
  onClose,
  open,
  draft,
}: {
  recordId: string;
  contextId?: string;
  /** The conversation shown, or `null` to start a new one. */
  conversationId: string | null;
  onConversationChange: (conversationId: string | null) => void;
  /** Shows the record's conversations; absent where they are not listed. */
  onShowAll?: () => void;
  /** Right edge of the drawer, to open it beside another panel. */
  offset?: string;
  onClose: () => void;
  open: boolean;
  draft?: DraftContext;
}) => {
  const { t } = useTranslation();
  const [renameOpen, setRenameOpen] = useState(false);
  const conversation = useQuery({
    ...conversationOptions(conversationId),
    enabled: open,
    refetchInterval: (query) =>
      titlePollInterval(query.state.data ? [query.state.data] : undefined),
  });
  const current = conversationId ? conversation.data : undefined;
  const client = useQueryClient();
  const session = useQuery({ ...sessionOptions(), enabled: open });
  // Chosen before the first message, then switched on for the new conversation.
  const [autoApproveOnStart, setAutoApproveOnStart] = useState(false);
  const autoApproval = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      setConversationAutoApproval(id, enabled),
    onSuccess: (_, { id }) =>
      client.invalidateQueries({ queryKey: conversationOptions(id).queryKey }),
  });
  const autoApprovalUser = current?.auto_approve_user_id ?? null;
  const ownAutoApproval =
    autoApprovalUser !== null && autoApprovalUser === session.data?.user_id;
  // Only someone who can edit the record may let the agent do so unasked.
  const autoApprovalSwitch = draft ? (
    conversationId ? (
      current && (
        <AutoApprovalSwitch
          checked={ownAutoApproval}
          disabled={autoApproval.isPending || !session.data}
          heldByOther={autoApprovalUser !== null && !ownAutoApproval}
          onChange={(enabled) =>
            autoApproval.mutate({ id: conversationId, enabled })
          }
        />
      )
    ) : (
      <AutoApprovalSwitch
        checked={autoApproveOnStart}
        onChange={setAutoApproveOnStart}
      />
    )
  ) : undefined;
  // The agent proposes form values only for the context it was asked about.
  const conversationDraft =
    draft && current?.context_id === draft.contextId ? draft : undefined;
  const closePanel = () => {
    setRenameOpen(false);
    onClose();
  };
  const start = async (content: string) => {
    const created = await createConversation(
      conversationTitleFromFirstMessage(content),
      { record_id: recordId, context_id: contextId },
    );
    if (autoApproveOnStart) await setConversationAutoApproval(created.id, true);
    return created.id;
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
            {conversationId
              ? current?.title || t('agents.untitledConversation')
              : t('records.askAboutRecord')}
          </Typography>
          {conversationId && current && (
            <Tooltip title={t('agents.renameConversation')}>
              <IconButton
                aria-label={t('agents.renameConversation')}
                onClick={() => setRenameOpen(true)}
              >
                <PencilIcon />
              </IconButton>
            </Tooltip>
          )}
          {conversationId && (
            <Tooltip title={t('agents.newConversation')}>
              <IconButton
                aria-label={t('agents.newConversation')}
                onClick={() => onConversationChange(null)}
              >
                <PlusIcon />
              </IconButton>
            </Tooltip>
          )}
          <IconButton aria-label={t('common.close')} onClick={closePanel}>
            <XIcon />
          </IconButton>
        </Box>
        {onShowAll && (
          <Box>
            <Button onClick={onShowAll} size="small" startIcon={<ListIcon />}>
              {t('records.agentConversations.showAll')}
            </Button>
          </Box>
        )}
        <ApiErrorAlert error={autoApproval.error} />
        {renameOpen && conversationId && current && (
          <RenameConversationDialog
            conversationId={conversationId}
            initialTitle={current.title}
            onClose={() => setRenameOpen(false)}
          />
        )}
        <ApiErrorAlert error={conversationId ? conversation.error : null} />
        {open &&
          (conversationId ? (
            <ConversationPanel
              key={conversationId}
              conversationId={conversationId}
              draft={conversationDraft}
              // The agent may have changed the record, with or without asking.
              onRunFinished={() => void invalidateRecord(client, recordId)}
              settings={autoApprovalSwitch}
              surface={composerSurfaces.paper}
            />
          ) : (
            <StartConversationPanel
              createConversation={start}
              draft={draft}
              empty={
                <Typography color="text.secondary" variant="body2">
                  {t('records.agentConversations.startHint')}
                </Typography>
              }
              onStarted={onConversationChange}
              settings={autoApprovalSwitch}
              surface={composerSurfaces.paper}
            />
          ))}
      </Box>
    </Drawer>
  );
};
