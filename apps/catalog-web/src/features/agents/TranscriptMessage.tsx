import { Avatar, Box, Chip, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { AgentIcon } from '../../components/systemIcons';
import { fileDownloadUrl } from '../files/api';
import {
  avatarSize,
  messageBubbleRadius,
  messageRoles,
  userMessageMaxWidth,
} from './constants';
import { ConversationMessageContent } from './ConversationMessageContent';
import { DraftProposal } from './DraftProposal';
import { parseDraftProposal } from './parseDraftProposal';
import type { ConversationMessage } from './schemas';

export const TranscriptMessage = ({
  getDraftValues,
  message,
  onApplyDraft,
}: {
  getDraftValues?: () => Record<string, string>;
  message: ConversationMessage;
  onApplyDraft?: (fields: Record<string, string>) => void;
}) => {
  const { t } = useTranslation();
  const isUser = message.role === messageRoles.user;
  const roleLabel = isUser ? t('agents.you') : t('agents.assistant');
  const proposal =
    onApplyDraft && message.role === messageRoles.assistant
      ? parseDraftProposal(message.content)
      : null;
  return (
    <Stack
      direction="row"
      spacing={1.5}
      sx={{
        alignItems: 'flex-start',
        alignSelf: isUser ? 'flex-end' : 'stretch',
        flexDirection: isUser ? 'row-reverse' : 'row',
        maxWidth: isUser ? userMessageMaxWidth : '100%',
      }}
    >
      <Avatar
        aria-label={roleLabel}
        sx={{
          bgcolor: isUser ? 'text.primary' : 'primary.main',
          height: avatarSize,
          mt: 0.25,
          width: avatarSize,
        }}
      >
        {isUser ? t('agents.you').slice(0, 1) : <AgentIcon fontSize="small" />}
      </Avatar>
      <Box
        sx={{
          bgcolor: isUser ? 'action.hover' : 'transparent',
          borderRadius: isUser ? messageBubbleRadius : 0,
          minWidth: 0,
          px: isUser ? 2 : 0,
          py: isUser ? 1.25 : 0,
          width: isUser ? 'fit-content' : '100%',
        }}
      >
        <Typography
          color="text.secondary"
          sx={{ display: 'block', mb: 0.5 }}
          variant="caption"
        >
          {roleLabel}
        </Typography>
        <ConversationMessageContent
          content={message.content}
          messageRole={message.role}
        />
        {proposal && onApplyDraft && (
          <DraftProposal
            getDraftValues={getDraftValues}
            onApply={onApplyDraft}
            proposal={proposal}
          />
        )}
        {message.attachments.length > 0 && (
          <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap', mt: 1 }}>
            {message.attachments.map((attachment) => (
              <Chip
                clickable
                component="a"
                href={fileDownloadUrl(attachment.id)}
                key={attachment.id}
                label={attachment.filename}
                size="small"
              />
            ))}
          </Stack>
        )}
      </Box>
    </Stack>
  );
};
