import { Box, Tab, Tabs, TextField } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { MarkdownContent } from './MarkdownContent';
import { FieldHint } from '../../components/FieldHint';
import { useTabAccessibility } from '../../components/useTabAccessibility';

export const MarkdownEditor = ({
  label,
  value,
  onChange,
  disabled,
  required,
  error,
  helperText,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  disabled: boolean;
  required?: boolean;
  error?: string;
  helperText?: string;
}) => {
  const { t } = useTranslation();
  const tabId = useTabAccessibility();
  const [tab, setTab] = useState(0);
  return (
    <Box sx={{ minWidth: 0 }}>
      <Tabs
        value={tab}
        onChange={(_, next: number) => setTab(next)}
        aria-label={label}
      >
        {[t('markdown.write'), t('markdown.preview')].map((title, index) => (
          <Tab key={index} label={title} {...tabId.tab(index)} />
        ))}
      </Tabs>
      <Box hidden={tab !== 0} {...tabId.panel(0)} sx={{ pt: 2 }}>
        <TextField
          fullWidth
          multiline
          minRows={6}
          label={label}
          value={value}
          disabled={disabled}
          required={required}
          error={Boolean(error)}
          helperText={
            error ?? (helperText && <FieldHint>{helperText}</FieldHint>)
          }
          onChange={(event) => {
            if (!disabled) onChange(event.target.value);
          }}
        />
      </Box>
      <Box hidden={tab !== 1} {...tabId.panel(1)} sx={{ pt: 2 }}>
        {tab === 1 && <MarkdownContent value={value || t('records.notSet')} />}
      </Box>
    </Box>
  );
};
