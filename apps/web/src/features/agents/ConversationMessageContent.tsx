import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Box,
  Chip,
  Link,
  Stack,
  Typography,
} from '@mui/material';
import ReactMarkdown from 'react-markdown';
import { bulkEditToolName } from './bulkEdits';
import { bulkEditResultSchema } from './schemas';
import { BulkEditResult } from './BulkEditResult';
import { useTranslation } from 'react-i18next';
import '../../i18n';
import { JsonBlock } from '../../components/JsonValue';
import { messageRoles, uuidPattern } from './constants';

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const JsonDetails = ({
  children,
  label,
}: {
  children: unknown;
  label: string;
}) => (
  <Accordion disableGutters elevation={0} sx={{ bgcolor: 'action.hover' }}>
    <AccordionSummary>{label}</AccordionSummary>
    <AccordionDetails>
      <JsonBlock value={children} />
    </AccordionDetails>
  </Accordion>
);

const ToolCall = ({ call }: { call: unknown }) => {
  const { t } = useTranslation();
  const functionCall =
    isRecord(call) && isRecord(call.function) ? call.function : null;
  const name =
    typeof functionCall?.name === 'string'
      ? functionCall.name
      : t('agents.unknownTool');

  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
        <Chip label={t('agents.toolCall')} size="small" />
        <Typography variant="body2">{name}</Typography>
      </Stack>
      <JsonDetails label={t('agents.showCallJson')}>{call}</JsonDetails>
    </Stack>
  );
};

const recordPreviewPath = new RegExp(`^/records/${uuidPattern}$`, 'i');
const savedSearchPath = new RegExp(`^/\\?savedView=${uuidPattern}$`, 'i');
const isAttricatLink = (url: string) =>
  recordPreviewPath.test(url) || savedSearchPath.test(url);

const ToolResult = ({ content }: { content: Record<string, unknown> }) => {
  const { t } = useTranslation();
  const name =
    typeof content.name === 'string' ? content.name : t('agents.unknownTool');
  const result = content.result;
  const bulkResult =
    name === bulkEditToolName
      ? bulkEditResultSchema.safeParse(result)
      : undefined;

  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
        <Chip color="success" label={t('agents.toolResult')} size="small" />
        <Typography variant="body2">{name}</Typography>
      </Stack>
      {isRecord(result) &&
        typeof result.url === 'string' &&
        isAttricatLink(result.url) && (
          <Link href={result.url} target="_blank" rel="noopener noreferrer">
            {typeof result.name === 'string'
              ? result.name
              : typeof result.record_id === 'string'
                ? result.record_id
                : result.url}
          </Link>
        )}
      {bulkResult?.success && <BulkEditResult result={bulkResult.data} />}
      <JsonDetails label={t('agents.showResultJson')}>{result}</JsonDetails>
    </Stack>
  );
};

const markdownStyles = {
  '& > :first-of-type': { mt: 0 },
  '& > :last-child': { mb: 0 },
  '& code': { bgcolor: 'action.hover', borderRadius: 0.5, px: 0.5 },
  '& pre': { bgcolor: 'action.hover', overflow: 'auto', p: 1 },
  '& pre code': { bgcolor: 'transparent', p: 0 },
} as const;

export const ConversationMessageContent = ({
  content,
  messageRole,
}: {
  content: unknown;
  messageRole: string;
}) => {
  if (isRecord(content) && isRecord(content.draft_proposal)) {
    return (
      <Typography>
        {typeof content.draft_proposal.explanation === 'string'
          ? content.draft_proposal.explanation
          : ''}
      </Typography>
    );
  }
  if (isRecord(content) && Array.isArray(content.tool_calls)) {
    return (
      <Stack spacing={1} sx={{ mt: 1 }}>
        {content.tool_calls.map((call, index) => (
          <ToolCall
            call={call}
            key={
              isRecord(call) && typeof call.id === 'string' ? call.id : index
            }
          />
        ))}
      </Stack>
    );
  }

  if (
    messageRole === messageRoles.tool &&
    isRecord(content) &&
    'tool_call_id' in content
  ) {
    return <ToolResult content={content} />;
  }

  if (typeof content === 'string' && messageRole === messageRoles.assistant) {
    return (
      <Box sx={markdownStyles}>
        <ReactMarkdown
          components={{
            a: ({ href, children }) =>
              href && isAttricatLink(href) ? (
                <Link href={href} target="_blank" rel="noopener noreferrer">
                  {children}
                </Link>
              ) : (
                <a href={href}>{children}</a>
              ),
          }}
        >
          {content}
        </ReactMarkdown>
      </Box>
    );
  }

  if (typeof content !== 'string') return <JsonBlock value={content} />;

  return (
    <Typography
      component="pre"
      sx={{ fontFamily: 'inherit', m: 0, whiteSpace: 'pre-wrap' }}
    >
      {content}
    </Typography>
  );
};
