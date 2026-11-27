import { useForm } from '@tanstack/react-form';
import { useMutation } from '@tanstack/react-query';
import { Stack, TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { FormDialog } from '../../components/FormDialog';
import { monoFontFamily } from '../../app/theme';
import { extensionSecretLimits, extensionSecretNamePattern } from './constants';
import { putWorkspaceExtensionSecret } from './managementApi';
import { utf8ByteLength } from './utf8';

type Props = {
  /** Names already stored; a new secret cannot silently replace one. */
  existingNames: ReadonlySet<string>;
  onClose: () => void;
  onSaved: (name: string) => void;
  /** The secret whose value is replaced; omitted to add a new secret. */
  replacing?: string;
};

/**
 * Adds a workspace extension secret or replaces the value of one. The value
 * is write-only: it is never shown again, so the field always starts empty.
 */
export const ExtensionSecretDialog = ({
  existingNames,
  onClose,
  onSaved,
  replacing,
}: Props) => {
  const { t } = useTranslation();
  const save = useMutation({
    mutationFn: putWorkspaceExtensionSecret,
    onSuccess: (_, { name }) => onSaved(name),
  });
  const form = useForm({
    defaultValues: { name: replacing ?? '', value: '' },
    onSubmit: ({ value }) => save.mutate(value),
  });
  const validateName = (name: string) => {
    if (replacing) return undefined;
    if (!name) return t('validation.required');
    if (
      !extensionSecretNamePattern.test(name) ||
      name.length > extensionSecretLimits.nameBytes
    )
      return t('extensions.secretNameInvalid', {
        max: extensionSecretLimits.nameBytes,
      });
    if (existingNames.has(name)) return t('extensions.secretNameTaken');
    return undefined;
  };
  const validateValue = (value: string) => {
    if (!value) return t('validation.required');
    if (utf8ByteLength(value) > extensionSecretLimits.valueBytes)
      return t('extensions.secretValueTooLarge', {
        max: extensionSecretLimits.valueBytes,
      });
    return undefined;
  };

  return (
    <FormDialog
      description={t('extensions.secretValueWriteOnly')}
      error={save.error}
      isPending={save.isPending}
      onClose={onClose}
      onSubmit={() => void form.handleSubmit()}
      submitLabel={
        replacing
          ? t('extensions.replaceSecretValue')
          : t('extensions.addSecret')
      }
      title={
        replacing
          ? t('extensions.replaceSecretTitle', { name: replacing })
          : t('extensions.addSecret')
      }
    >
      <Stack spacing={2} sx={{ pt: 2 }}>
        {!replacing && (
          <form.Field
            name="name"
            validators={{
              onBlur: ({ value }) => validateName(value),
              onSubmit: ({ value }) => validateName(value),
            }}
          >
            {(field) => (
              <TextField
                autoComplete="off"
                error={field.state.meta.errors.length > 0}
                helperText={
                  field.state.meta.errors[0] ?? t('extensions.secretNameHelp')
                }
                label={t('extensions.secretName')}
                onBlur={field.handleBlur}
                onChange={(event) => field.handleChange(event.target.value)}
                required
                slotProps={{
                  htmlInput: {
                    maxLength: extensionSecretLimits.nameBytes,
                    spellCheck: false,
                    sx: { fontFamily: monoFontFamily },
                  },
                }}
                value={field.state.value}
              />
            )}
          </form.Field>
        )}
        <form.Field
          name="value"
          validators={{ onSubmit: ({ value }) => validateValue(value) }}
        >
          {(field) => (
            <TextField
              autoComplete="new-password"
              error={field.state.meta.errors.length > 0}
              helperText={field.state.meta.errors[0]}
              label={
                replacing
                  ? t('extensions.newSecretValue')
                  : t('extensions.secretValue')
              }
              onBlur={field.handleBlur}
              onChange={(event) => field.handleChange(event.target.value)}
              required
              type="password"
              value={field.state.value}
            />
          )}
        </form.Field>
      </Stack>
    </FormDialog>
  );
};
