import AttachFileOutlinedIcon from '@mui/icons-material/AttachFileOutlined';
import ArrowUpwardIcon from '@mui/icons-material/ArrowUpward';
import {
  Chip,
  CircularProgress,
  IconButton,
  Paper,
  Stack,
  TextField,
  Tooltip,
} from '@mui/material';
import { useRef } from 'react';
import { useTranslation } from 'react-i18next';
import {
  composerBorderRadius,
  composerMaxRows,
  progressSize,
} from './constants';

export type MessageInputBoxProps = {
  content: string;
  files: File[];
  isPending: boolean;
  minRows: number;
  onContentChange: (content: string) => void;
  onFilesChange: (files: File[]) => void;
  onRemoveFile: (file: File) => void;
  onSubmit: () => void;
};

const attachmentKey = (file: File) =>
  `${file.name}:${file.size}:${file.lastModified}`;

export const MessageInputBox = ({
  content,
  files,
  isPending,
  minRows,
  onContentChange,
  onFilesChange,
  onRemoveFile,
  onSubmit,
}: MessageInputBoxProps) => {
  const { t } = useTranslation();
  const attachmentInput = useRef<HTMLInputElement>(null);
  return (
    <Paper
      elevation={0}
      sx={{
        border: 1,
        borderColor: 'divider',
        borderRadius: composerBorderRadius,
        p: 0.75,
      }}
    >
      <TextField
        disabled={isPending}
        fullWidth
        hiddenLabel
        maxRows={composerMaxRows}
        minRows={minRows}
        multiline
        onChange={(event) => onContentChange(event.target.value)}
        onKeyDown={(event) => {
          if (
            event.key === 'Enter' &&
            !event.shiftKey &&
            !event.nativeEvent.isComposing
          ) {
            event.preventDefault();
            onSubmit();
          }
        }}
        placeholder={t('agents.messagePlaceholder')}
        slotProps={{ htmlInput: { 'aria-label': t('agents.message') } }}
        sx={{ '& .MuiOutlinedInput-notchedOutline': { border: 0 } }}
        value={content}
      />
      <input
        disabled={isPending}
        hidden
        multiple
        onChange={(event) => {
          onFilesChange(Array.from(event.target.files ?? []));
          event.target.value = '';
        }}
        ref={attachmentInput}
        type="file"
      />
      {files.length > 0 && (
        <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap', px: 1 }}>
          {files.map((file) => (
            <Chip
              key={attachmentKey(file)}
              label={file.name}
              onDelete={isPending ? undefined : () => onRemoveFile(file)}
              size="small"
            />
          ))}
        </Stack>
      )}
      <Stack
        direction="row"
        sx={{
          alignItems: 'center',
          justifyContent: 'space-between',
          mt: 0.5,
        }}
      >
        <Tooltip title={t('agents.addFiles')}>
          <IconButton
            aria-label={t('agents.addFiles')}
            disabled={isPending}
            onClick={() => attachmentInput.current?.click()}
          >
            <AttachFileOutlinedIcon />
          </IconButton>
        </Tooltip>
        <Tooltip title={t('agents.send')}>
          <span>
            <IconButton
              aria-label={t('agents.send')}
              color="primary"
              disabled={(!content.trim() && files.length === 0) || isPending}
              type="submit"
            >
              {isPending ? (
                <CircularProgress enableTrackSlot size={progressSize} />
              ) : (
                <ArrowUpwardIcon />
              )}
            </IconButton>
          </span>
        </Tooltip>
      </Stack>
    </Paper>
  );
};
