import {
  FormControl,
  FormHelperText,
  InputLabel,
  MenuItem,
  Paper,
  Select,
  Stack,
  Typography,
} from '@mui/material';
import { useId, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { Blueprint } from '../blueprints/api';
import type { AttributeContext } from '../contexts/api';
import { contextLabel } from '../contexts/defaultContext';
import { lexiconText } from '../lexicon/lexicon';
import { ALL_CONTEXTS, RULE_SCOPE_SELECT_MIN_WIDTH } from './constants';

/** The blueprint revision and context a rule checks. */
export type RuleScope = {
  blueprintId?: string;
  blueprintVersion?: number;
  contextId: string | null;
};

const ScopeSelect = ({
  disabled,
  error,
  helperText,
  label,
  onChange,
  options,
  value,
}: {
  disabled?: boolean;
  error?: boolean;
  helperText?: ReactNode;
  label: string;
  onChange: (value: string) => void;
  options: readonly { label: string; value: string }[];
  value: string;
}) => {
  const labelId = useId();
  const helperId = useId();
  return (
    <FormControl
      disabled={disabled}
      error={error}
      size="small"
      sx={{ minWidth: RULE_SCOPE_SELECT_MIN_WIDTH }}
    >
      <InputLabel id={labelId} shrink>
        {label}
      </InputLabel>
      <Select
        aria-describedby={helperText ? helperId : undefined}
        displayEmpty
        label={label}
        labelId={labelId}
        notched
        onChange={(event) => onChange(event.target.value)}
        value={value}
      >
        {options.map((option) => (
          <MenuItem key={option.value} value={option.value}>
            {option.label}
          </MenuItem>
        ))}
      </Select>
      {helperText && (
        <FormHelperText id={helperId}>{helperText}</FormHelperText>
      )}
    </FormControl>
  );
};

/**
 * Chooses what a new rule checks: a record blueprint, one of its published
 * revisions, and optionally a single context. Revisions keep these.
 */
export const RuleScopeFields = ({
  blueprints,
  contexts,
  disabled,
  onChange,
  publishedRevisions,
  scope,
}: {
  /** Record blueprints, latest revision of each. */
  blueprints: readonly Blueprint[];
  contexts: readonly AttributeContext[];
  disabled: boolean;
  onChange: (scope: RuleScope) => void;
  /** Published revisions of the chosen blueprint, newest first. */
  publishedRevisions: readonly Blueprint[] | undefined;
  scope: RuleScope;
}) => {
  const { t } = useTranslation();
  const noPublishedRevision =
    scope.blueprintId !== undefined && publishedRevisions?.length === 0;
  return (
    <Paper component="section" sx={{ mt: 3, p: 2 }} variant="outlined">
      <Typography color="text.secondary" variant="body2">
        {t('rules.editor.scopeDescription')}
      </Typography>
      <Stack
        direction={{ sm: 'row', xs: 'column' }}
        spacing={2}
        sx={{ alignItems: { sm: 'flex-start' }, mt: 3 }}
      >
        <ScopeSelect
          disabled={disabled}
          error={noPublishedRevision}
          helperText={
            noPublishedRevision
              ? t('rules.editor.noPublishedRevision')
              : undefined
          }
          label={t('rules.editor.blueprint')}
          onChange={(blueprintId) =>
            onChange({ ...scope, blueprintId, blueprintVersion: undefined })
          }
          options={blueprints
            .map((blueprint) => ({
              label: lexiconText(blueprint.name),
              value: blueprint.id,
            }))
            .sort((a, b) => a.label.localeCompare(b.label))}
          value={scope.blueprintId ?? ''}
        />
        <ScopeSelect
          disabled={disabled || !publishedRevisions?.length}
          label={t('rules.editor.blueprintRevision')}
          onChange={(version) =>
            onChange({ ...scope, blueprintVersion: Number(version) })
          }
          options={(publishedRevisions ?? []).map((revision) => ({
            label: t('rules.editor.revisionOption', {
              version: revision.version,
            }),
            value: String(revision.version),
          }))}
          value={
            scope.blueprintVersion === undefined
              ? ''
              : String(scope.blueprintVersion)
          }
        />
        <ScopeSelect
          disabled={disabled}
          label={t('rules.filters.context')}
          onChange={(contextId) =>
            onChange({
              ...scope,
              contextId: contextId === ALL_CONTEXTS ? null : contextId,
            })
          }
          options={[
            {
              label: t('rules.recordFindings.allContexts'),
              value: ALL_CONTEXTS,
            },
            ...contexts.map((context) => ({
              label: contextLabel(t, context.code),
              value: context.id,
            })),
          ]}
          value={scope.contextId ?? ALL_CONTEXTS}
        />
      </Stack>
    </Paper>
  );
};
