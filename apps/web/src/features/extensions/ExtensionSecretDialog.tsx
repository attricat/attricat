import { useForm } from '@tanstack/react-form';
import { useMutation } from '@tanstack/react-query';
import { Stack, TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { FormDialog } from '../../components/FormDialog';
import { monoFontFamily } from '../../app/theme';
import { extensionSecretLimits, extensionSecretNamePattern } from './constants';
import { putExtensionSecret } from './managementApi';
import { utf8ByteLength } from './utf8';

type Props = {
  /** Names the extension declares or has set; a new name cannot reuse one. */
  existingNames: ReadonlySet<string>;
  extensionId: string;
  /** The secret whose value is set; omitted to add a secret by a new name. */
  name?: string;
  onClose: () => void;
  onSaved: (name: string) => void;
  /** Whether the named secret already has a value this one replaces. */
  replacing: boolean;
};

/**
 * Sets the value of one of an extension's secrets, or adds a secret by a name
 * its manifest does not declare. The value is write-only: it is never shown
 * again, so the field always starts empty.
 */
export const ExtensionSecretDialog = ({
  existingNames,
  extensionId,
  name: fixedName,
  onClose,
  onSaved,
  replacing,
}: Props) => {
  const { t } = useTranslation();
  const save = useMutation({
    // The section shows its own message.
    meta: { toast: false },
    // The value is write-only, so the mutation leaves the cache as soon as
    // the dialog closes instead of keeping it for the default five minutes.
    gcTime: 0,
    mutationFn: putExtensionSecret,
    onSuccess: (_, { name }) => onSaved(name),
  });
  const form = useForm({
    defaultValues: { name: fixedName ?? '', value: '' },
    onSubmit: ({ value }) => save.mutate({ extensionId, ...value }),
  });
  const validateName = (name: string) => {
    if (fixedName) return undefined;
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
          : fixedName
            ? t('extensions.setSecretValue')
            : t('extensions.addSecret')
      }
      title={
        fixedName
          ? t(
              replacing
                ? 'extensions.replaceSecretTitle'
                : 'extensions.setSecretTitle',
              { name: fixedName },
            )
          : t('extensions.addSecret')
      }
    >
      <Stack spacing={2} sx={{ pt: 2 }}>
        {!fixedName && (
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
