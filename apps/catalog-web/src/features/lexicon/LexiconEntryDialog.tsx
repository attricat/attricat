import { useRef } from 'react';
import { useForm } from '@tanstack/react-form';
import { useTranslation } from 'react-i18next';
import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  MenuItem,
  Stack,
  TextField,
} from '@mui/material';
import {
  LEXICON_RESERVED_CHARACTERS,
  MAX_LEXICON_CONTEXT_LENGTH,
  MAX_LEXICON_KEY_LENGTH,
  MAX_LEXICON_TEXT_LENGTH,
  LEXICON_MANAGEMENT_NAMESPACES,
} from './constants';
import type { LexiconEntryDraft } from './entries';
import { languageLabel, pluralCategories, pluralExamples } from './languages';
import { normalizeLexiconTerm } from './references';
import { useLexiconMutations } from './useLexiconMutations';

/**
 * Adds or edits one translation. In edit mode the identity (key, context,
 * plural form) is fixed; only the text changes.
 */
export const LexiconEntryDialog = ({
  initial,
  language,
  mode,
  onClose,
}: {
  initial: LexiconEntryDraft;
  language: string;
  mode: 'add' | 'edit';
  onClose: () => void;
}) => {
  const { i18n, t } = useTranslation(LEXICON_MANAGEMENT_NAMESPACES);
  const keyRef = useRef<HTMLInputElement>(null);
  const textRef = useRef<HTMLInputElement>(null);
  const { save } = useLexiconMutations();
  const editing = mode === 'edit';
  // A prefilled key (from the coverage list) means only the text is missing.
  const focusText = editing || Boolean(initial.key);
  const validateTerm = (
    value: string,
    maxLength: number,
    required: boolean,
  ) => {
    const term = normalizeLexiconTerm(value);
    if (!term) return required ? t('lexicon.keyRequired') : undefined;
    if (term.length > maxLength)
      return t('lexicon.termTooLong', { count: maxLength });
    if (LEXICON_RESERVED_CHARACTERS.test(term))
      return t('lexicon.reservedCharacters');
    return undefined;
  };
  const validateText = (value: string) => {
    if (!value.trim()) return t('lexicon.textRequired');
    if (value.length > MAX_LEXICON_TEXT_LENGTH)
      return t('lexicon.termTooLong', { count: MAX_LEXICON_TEXT_LENGTH });
    return undefined;
  };
  const form = useForm({
    defaultValues: initial,
    onSubmit: ({ value }) =>
      save.mutate(
        {
          key: normalizeLexiconTerm(value.key),
          context: normalizeLexiconTerm(value.context) || null,
          language,
          plural_category: value.plural_category,
          text: value.text,
        },
        { onSuccess: onClose },
      ),
  });
  const categories = pluralCategories(language);
  return (
    <Dialog
      fullWidth
      maxWidth="sm"
      onClose={() => {
        if (!save.isPending) onClose();
      }}
      open
      slotProps={{
        transition: {
          onEntered: () => {
            (focusText ? textRef : keyRef).current?.focus();
          },
        },
      }}
    >
      <Box
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          void form.handleSubmit();
        }}
      >
        <DialogTitle>
          {editing ? t('lexicon.editTranslation') : t('lexicon.addTranslation')}
        </DialogTitle>
        <DialogContent>
          <DialogContentText sx={{ mb: 2 }}>
            {t('lexicon.dialogLanguage', {
              language: languageLabel(language, i18n.language),
            })}
          </DialogContentText>
          <Stack spacing={2} sx={{ pt: 1 }}>
            <form.Field
              name="key"
              validators={{
                onChange: ({ value }) =>
                  validateTerm(value, MAX_LEXICON_KEY_LENGTH, true),
                onSubmit: ({ value }) =>
                  validateTerm(value, MAX_LEXICON_KEY_LENGTH, true),
              }}
            >
              {(field) => (
                <TextField
                  autoComplete="off"
                  disabled={editing}
                  error={field.state.meta.errors.length > 0}
                  fullWidth
                  helperText={
                    field.state.meta.errors[0] ?? t('lexicon.keyHelp')
                  }
                  inputRef={keyRef}
                  label={t('lexicon.key')}
                  onBlur={field.handleBlur}
                  onChange={(event) => field.handleChange(event.target.value)}
                  required
                  value={field.state.value}
                />
              )}
            </form.Field>
            <form.Field
              name="context"
              validators={{
                onChange: ({ value }) =>
                  validateTerm(value, MAX_LEXICON_CONTEXT_LENGTH, false),
              }}
            >
              {(field) => (
                <TextField
                  autoComplete="off"
                  disabled={editing}
                  error={field.state.meta.errors.length > 0}
                  fullWidth
                  helperText={
                    field.state.meta.errors[0] ?? t('lexicon.contextHelp')
                  }
                  label={t('lexicon.context')}
                  onBlur={field.handleBlur}
                  onChange={(event) => field.handleChange(event.target.value)}
                  value={field.state.value}
                />
              )}
            </form.Field>
            <form.Field name="plural_category">
              {(field) => (
                <TextField
                  disabled={editing}
                  fullWidth
                  helperText={t('lexicon.pluralCategoryHelp')}
                  label={t('lexicon.pluralCategory')}
                  onChange={(event) => field.handleChange(event.target.value)}
                  select
                  value={field.state.value}
                >
                  {categories.map((category) => (
                    <MenuItem key={category} value={category}>
                      {t('lexicon.pluralCategoryOption', {
                        category,
                        examples: pluralExamples(language, category).join(', '),
                      })}
                    </MenuItem>
                  ))}
                </TextField>
              )}
            </form.Field>
            <form.Field
              name="text"
              validators={{
                onChange: ({ value }) => validateText(value),
                onSubmit: ({ value }) => validateText(value),
              }}
            >
              {(field) => (
                <TextField
                  autoComplete="off"
                  error={field.state.meta.errors.length > 0}
                  fullWidth
                  helperText={
                    field.state.meta.errors[0] ?? t('lexicon.textHelp')
                  }
                  inputRef={textRef}
                  label={t('lexicon.translation')}
                  multiline
                  onBlur={field.handleBlur}
                  onChange={(event) => field.handleChange(event.target.value)}
                  required
                  value={field.state.value}
                />
              )}
            </form.Field>
          </Stack>
          {save.error && (
            <Alert severity="error" sx={{ mt: 2 }}>
              {save.error.message}
            </Alert>
          )}
        </DialogContent>
        <DialogActions>
          <Button disabled={save.isPending} onClick={onClose}>
            {t('common.cancel')}
          </Button>
          <Button disabled={save.isPending} type="submit" variant="contained">
            {t('lexicon.saveTranslation')}
          </Button>
        </DialogActions>
      </Box>
    </Dialog>
  );
};
