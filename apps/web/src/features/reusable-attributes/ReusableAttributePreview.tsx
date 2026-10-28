import { Chip, Paper, Stack, Typography } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { valueForField } from '../records/attributeValues';
import { SandboxAttributeEditor } from '../blueprints/SandboxAttributeEditor';
import { sandboxValidation } from '../blueprints/sandboxValues';
import type { ReusableAttribute } from './api';
import { attributeForReusable } from './attributeForReusable';

/**
 * Shows a reusable attribute as records will: its field, in the same sandbox
 * the blueprint preview uses, starting from its default value.
 */
export const ReusableAttributePreview = ({
  reusable,
}: {
  reusable: ReusableAttribute;
}) => {
  const { t } = useTranslation();
  const attribute = attributeForReusable(reusable);
  const [fields, setFields] = useState<Record<string, string>>({});
  const value =
    fields[attribute.code] ??
    (reusable.default_value === null
      ? ''
      : valueForField(
          reusable.default_value as Parameters<typeof valueForField>[0],
        ));
  const validation = sandboxValidation([attribute], fields);
  const flags = [
    reusable.searchable && t('reusableAttributes.preview.searchable'),
    // Filters can name only searchable attributes.
    reusable.searchable &&
      reusable.facetable &&
      t('reusableAttributes.preview.facetable'),
    reusable.readonly && t('reusableAttributes.preview.readonly'),
    reusable.context_editable === 'default' &&
      t('reusableAttributes.preview.defaultContextOnly'),
  ].filter((flag): flag is string => Boolean(flag));

  return (
    <>
      <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
        {reusable.value_type} · {reusable.namespace}:{reusable.code}
      </Typography>
      {flags.length > 0 && (
        <Stack direction="row" sx={{ flexWrap: 'wrap', gap: 1, mt: 1 }}>
          {flags.map((flag) => (
            <Chip key={flag} label={flag} size="small" variant="outlined" />
          ))}
        </Stack>
      )}
      <Typography color="text.secondary" sx={{ mt: 2 }} variant="body2">
        {t('blueprints.sandboxDescription')}
      </Typography>
      <Paper sx={{ mt: 1, p: 2.5 }} variant="outlined">
        <SandboxAttributeEditor
          attribute={attribute}
          error={validation.fieldErrors[attribute.code]}
          onChange={(next) => setFields({ [attribute.code]: next })}
          value={value}
        />
      </Paper>
    </>
  );
};
