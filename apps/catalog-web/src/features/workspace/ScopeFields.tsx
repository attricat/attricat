import { useQuery } from '@tanstack/react-query';
import { Alert, MenuItem, TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { listGrantTargets, selectedScopeTarget, type ScopeType } from './api';
import { workspaceQueryKeys } from './queryKeys';

type ScopeFieldsProps = {
  onScopeChange: (scope: ScopeType) => void;
  onScopeTargetChange: (scopeTargetId: string) => void;
  scope: ScopeType;
  scopeTargetId: string;
  workspaceId?: string;
};

export const ScopeFields = ({
  onScopeChange,
  onScopeTargetChange,
  scope,
  scopeTargetId,
  workspaceId,
}: ScopeFieldsProps) => {
  const { t } = useTranslation();
  return (
    <>
      <TextField
        fullWidth
        label={t('workspace.scope')}
        onChange={(event) => {
          const nextScope = event.target.value as ScopeType;
          onScopeChange(nextScope);
          onScopeTargetChange(selectedScopeTarget(nextScope, '', workspaceId));
        }}
        select
        value={scope}
      >
        <MenuItem value="workspace">{t('workspace.entireWorkspace')}</MenuItem>
        <MenuItem value="blueprint_family">
          {t('workspace.blueprintFamily')}
        </MenuItem>
        <MenuItem value="context_subtree">
          {t('workspace.contextSubtree')}
        </MenuItem>
        <MenuItem value="entity">{t('workspace.entity')}</MenuItem>
      </TextField>
      <ScopeTargetField
        onScopeTargetChange={onScopeTargetChange}
        scope={scope}
        scopeTargetId={scopeTargetId}
      />
    </>
  );
};

const ScopeTargetField = ({
  onScopeTargetChange,
  scope,
  scopeTargetId,
}: Pick<
  ScopeFieldsProps,
  'onScopeTargetChange' | 'scope' | 'scopeTargetId'
>) => {
  const { t } = useTranslation();
  const targets = useQuery({
    enabled: scope !== 'workspace',
    queryKey: workspaceQueryKeys.grantTargets(scope),
    queryFn: () => listGrantTargets(scope),
  });
  if (scope === 'workspace') return null;
  const label =
    scope === 'blueprint_family'
      ? t('workspace.blueprintFamily')
      : scope === 'context_subtree'
        ? t('workspace.contextSubtree')
        : t('workspace.entity');
  return (
    <>
      {targets.isError && (
        <Alert severity="error">{targets.error.message}</Alert>
      )}
      <TextField
        fullWidth
        helperText={t('workspace.ownedTargets')}
        label={label}
        onChange={(event) => onScopeTargetChange(event.target.value)}
        select
        value={scopeTargetId}
      >
        {targets.data?.map((target) => (
          <MenuItem key={target.id} value={target.id}>
            {target.label}
          </MenuItem>
        ))}
      </TextField>
    </>
  );
};
