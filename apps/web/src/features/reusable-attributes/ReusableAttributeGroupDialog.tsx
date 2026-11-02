import { revisionStatuses } from '../../api/revisionStatus';
import { useForm } from '@tanstack/react-form';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Alert, MenuItem, Stack, TextField } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  createReusableAttributeGroup,
  updateReusableAttributeGroup,
  type ReusableAttribute,
  type ReusableAttributeGroup,
} from './api';
import { attachedReusableCode } from './attributeForReusable';
import {
  DEFAULT_GROUP_POSITION,
  GROUP_POSITION_STEP,
  MIN_GROUP_POSITION,
} from './constants';
import { reusableAttributeQueryKeys } from './queryKeys';
import { FormDialog } from '../../components/FormDialog';

export const ReusableAttributeGroupDialog = ({
  attributes,
  group,
  onClose,
}: {
  attributes: ReusableAttribute[];
  /** The group to edit; without one the dialog creates a group. */
  group?: ReusableAttributeGroup;
  onClose: () => void;
}) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [validationError, setValidationError] = useState<string>();
  const save = useMutation({
    mutationFn: ({
      code,
      ...input
    }: Parameters<typeof createReusableAttributeGroup>[0]) =>
      group
        ? updateReusableAttributeGroup(group.id, input)
        : createReusableAttributeGroup({ code, ...input }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: reusableAttributeQueryKeys.groups(),
      });
      onClose();
    },
  });
  const form = useForm({
    defaultValues: {
      code: group?.code ?? '',
      name: group?.name ?? '',
      position: group ? String(group.position) : DEFAULT_GROUP_POSITION,
      attributeCodes: group?.reusable_attribute_codes ?? [],
    },
    onSubmit: ({ value }) => {
      setValidationError(undefined);
      if (
        !value.code.trim() ||
        !value.name.trim() ||
        !value.attributeCodes.length
      ) {
        setValidationError(t('reusableAttributes.groupDialog.requiredFields'));
        return;
      }
      const position = Number(value.position);
      if (!Number.isInteger(position) || position < MIN_GROUP_POSITION) {
        setValidationError(t('reusableAttributes.groupDialog.invalidPosition'));
        return;
      }
      save.mutate({
        code: value.code.trim(),
        name: value.name.trim(),
        position,
        reusable_attribute_codes: value.attributeCodes,
      });
    },
  });
  const publishedCodes = [
    ...new Set(
      attributes
        .filter((attribute) => attribute.status === revisionStatuses.published)
        .map(attachedReusableCode),
    ),
  ].sort();
  return (
    <FormDialog
      error={validationError ? undefined : save.error}
      isPending={save.isPending}
      onClose={onClose}
      onSubmit={() => void form.handleSubmit()}
      submitDisabled={!publishedCodes.length}
      submitLabel={
        group
          ? t('reusableAttributes.groupDialog.save')
          : t('reusableAttributes.groupDialog.create')
      }
      title={
        group
          ? t('reusableAttributes.groupDialog.editTitle', { name: group.name })
          : t('reusableAttributes.groupDialog.title')
      }
    >
      <Stack spacing={2} sx={{ pt: 1 }}>
        <form.Field name="code">
          {(field) => (
            <TextField
              disabled={Boolean(group)}
              helperText={
                group && t('reusableAttributes.groupDialog.codeImmutable')
              }
              label={t('reusableAttributes.groupDialog.code')}
              onChange={(event) => field.handleChange(event.target.value)}
              required
              value={field.state.value}
            />
          )}
        </form.Field>
        <form.Field name="name">
          {(field) => (
            <TextField
              label={t('reusableAttributes.groupDialog.name')}
              onChange={(event) => field.handleChange(event.target.value)}
              required
              value={field.state.value}
            />
          )}
        </form.Field>
        <form.Field name="position">
          {(field) => (
            <TextField
              label={t('reusableAttributes.groupDialog.position')}
              onChange={(event) => field.handleChange(event.target.value)}
              slotProps={{
                htmlInput: {
                  min: MIN_GROUP_POSITION,
                  step: GROUP_POSITION_STEP,
                },
              }}
              type="number"
              value={field.state.value}
            />
          )}
        </form.Field>
        <form.Field name="attributeCodes">
          {(field) => (
            <TextField
              required
              helperText={t('reusableAttributes.groupDialog.attributesHelp')}
              label={t('reusableAttributes.groupDialog.attributes')}
              onChange={(event) => {
                // Autofill can set a multiple select to comma-joined text.
                const { value } = event.target;
                field.handleChange(
                  typeof value === 'string' ? value.split(',') : value,
                );
              }}
              select
              slotProps={{ select: { multiple: true } }}
              value={field.state.value}
            >
              {publishedCodes.map((code) => (
                <MenuItem key={code} value={code}>
                  {code}
                </MenuItem>
              ))}
            </TextField>
          )}
        </form.Field>
        {validationError && <Alert severity="error">{validationError}</Alert>}
      </Stack>
    </FormDialog>
  );
};
