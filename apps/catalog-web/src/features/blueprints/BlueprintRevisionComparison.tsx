import {
  Alert,
  Box,
  MenuItem,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { lazy, Suspense, type ComponentProps } from 'react';
import { useTranslation } from 'react-i18next';
import type { Blueprint } from './schemas';
import type { BlueprintRevisionComparison as Comparison } from './useBlueprintRevisionComparison';

const TomlDiffEditor = lazy(() =>
  import('./TomlDiffEditor').then(({ TomlDiffEditor }) => ({
    default: TomlDiffEditor,
  })),
);

const RevisionSelect = ({
  label,
  onChange,
  revisions,
  value,
}: {
  label: string;
  onChange: (version: number) => void;
  revisions: readonly Blueprint[];
  value: number | undefined;
}) => {
  const { t } = useTranslation();
  return (
    <TextField
      label={label}
      onChange={(event) => onChange(Number(event.target.value))}
      select
      value={value ?? ''}
    >
      {revisions.map((revision) => (
        <MenuItem key={revision.version} value={revision.version}>
          {t('blueprints.versionOption', {
            version: revision.version,
            status: t(`blueprints.revisionStatuses.${revision.status}`),
          })}
        </MenuItem>
      ))}
    </TextField>
  );
};

export const BlueprintRevisionComparison = ({
  comparison,
  revisions,
  ...panelProps
}: {
  comparison: Comparison;
  revisions: readonly Blueprint[];
} & Pick<ComponentProps<'section'>, 'aria-labelledby' | 'id' | 'role'>) => {
  const { t } = useTranslation();
  const { left, right } = comparison;
  const revisionTitle = ({ version, status }: Blueprint) =>
    t('blueprints.revisionTitle', {
      version,
      status: t(`blueprints.revisionStatuses.${status}`),
    });

  return (
    <Paper {...panelProps} component="section" sx={{ mt: 3, p: 2.5 }}>
      <Typography component="h2" variant="h6">
        {t('blueprints.compareDefinitions')}
      </Typography>
      <Typography color="text.secondary" sx={{ mt: 1 }}>
        {t('blueprints.compareDefinitionsDescription')}
      </Typography>
      <Stack direction={{ xs: 'column', sm: 'row' }} spacing={2} sx={{ mt: 2 }}>
        <RevisionSelect
          label={t('blueprints.leftVersion')}
          onChange={comparison.selectLeftVersion}
          revisions={revisions}
          value={comparison.leftVersion}
        />
        <RevisionSelect
          label={t('blueprints.rightVersion')}
          onChange={comparison.selectRightVersion}
          revisions={revisions}
          value={comparison.rightVersion}
        />
      </Stack>
      {(left.isError || right.isError) && (
        <Alert severity="error" sx={{ mt: 2 }}>
          {left.error?.message ?? right.error?.message}
        </Alert>
      )}
      <Box sx={{ mt: 3 }}>
        <Suspense
          fallback={<Typography>{t('blueprints.loadingEditor')}</Typography>}
        >
          {left.data && right.data && (
            <TomlDiffEditor
              modified={right.data.blueprint.definition}
              modifiedTitle={revisionTitle(right.data.blueprint)}
              original={left.data.blueprint.definition}
              originalTitle={revisionTitle(left.data.blueprint)}
            />
          )}
        </Suspense>
      </Box>
    </Paper>
  );
};
