import { Box, Tab, Tabs, TextField } from '@mui/material';
import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { MarkdownContent } from './MarkdownContent';

export const MarkdownEditor = ({
  label,
  value,
  onChange,
  disabled,
  error,
  helperText,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  disabled: boolean;
  error?: string;
  helperText?: string;
}) => {
  const { t } = useTranslation();
  const id = useId();
  const [tab, setTab] = useState(0);
  return (
    <Box sx={{ minWidth: 0 }}>
      <Tabs
        value={tab}
        onChange={(_, next: number) => setTab(next)}
        aria-label={label}
      >
        {[t('markdown.write'), t('markdown.preview')].map((title, index) => (
          <Tab
            key={index}
            label={title}
            id={`${id}-tab-${index}`}
            aria-controls={`${id}-panel-${index}`}
          />
        ))}
      </Tabs>
      <Box
        hidden={tab !== 0}
        role="tabpanel"
        id={`${id}-panel-0`}
        aria-labelledby={`${id}-tab-0`}
        sx={{ pt: 2 }}
      >
        <TextField
          fullWidth
          multiline
          minRows={6}
          label={label}
          value={value}
          disabled={disabled}
          error={Boolean(error)}
          helperText={error ?? helperText}
          onChange={(event) => {
            if (!disabled) onChange(event.target.value);
          }}
        />
      </Box>
      <Box
        hidden={tab !== 1}
        role="tabpanel"
        id={`${id}-panel-1`}
        aria-labelledby={`${id}-tab-1`}
        sx={{ pt: 2 }}
      >
        {tab === 1 && <MarkdownContent value={value || t('entities.notSet')} />}
      </Box>
    </Box>
  );
};
