import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Box,
  Chip,
  Stack,
  Typography,
} from '@mui/material';
import ReactMarkdown from 'react-markdown';

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const formatJson = (value: unknown) =>
  JSON.stringify(value, null, 2) ?? String(value);

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
      <Box
        component="pre"
        sx={{ m: 0, overflow: 'auto', whiteSpace: 'pre-wrap' }}
      >
        {formatJson(children)}
      </Box>
    </AccordionDetails>
  </Accordion>
);

const ToolCall = ({ call }: { call: unknown }) => {
  const functionCall =
    isRecord(call) && isRecord(call.function) ? call.function : null;
  const name =
    typeof functionCall?.name === 'string' ? functionCall.name : 'unknown tool';

  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
        <Chip label="Tool call" size="small" />
        <Typography variant="body2">{name}</Typography>
      </Stack>
      <JsonDetails label="Show call JSON">{call}</JsonDetails>
    </Stack>
  );
};

const ToolResult = ({ content }: { content: Record<string, unknown> }) => {
  const name = typeof content.name === 'string' ? content.name : 'unknown tool';
  const result = content.result;

  return (
    <Stack spacing={1}>
      <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
        <Chip color="success" label="Tool result" size="small" />
        <Typography variant="body2">{name}</Typography>
      </Stack>
      <JsonDetails label="Show result JSON">{result}</JsonDetails>
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
  role,
}: {
  content: unknown;
  role: string;
}) => {
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

  if (role === 'tool' && isRecord(content) && 'tool_call_id' in content) {
    return <ToolResult content={content} />;
  }

  if (typeof content === 'string' && role === 'assistant') {
    return (
      <Box sx={markdownStyles}>
        <ReactMarkdown>{content}</ReactMarkdown>
      </Box>
    );
  }

  return (
    <Typography
      component="pre"
      sx={{ fontFamily: 'inherit', m: 0, whiteSpace: 'pre-wrap' }}
    >
      {typeof content === 'string' ? content : formatJson(content)}
    </Typography>
  );
};
