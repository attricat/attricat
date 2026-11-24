import { Button, MenuItem, Stack, TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { IconLabel } from '../../../components/IconLabel';
import { SearchField } from '../../../components/SearchField';
import { BlueprintIcon, ExplorerIcon } from '../../../components/systemIcons';

/**
 * Switches a relationship picker between the blueprints its relationship
 * allows; nothing is shown when there is only one.
 */
export const TargetBlueprintSelect = ({
  blueprintName,
  onChange,
  targets,
  value,
}: {
  blueprintName: (code: string) => string;
  onChange: (code: string) => void;
  targets: readonly string[];
  value: string | undefined;
}) => {
  const { t } = useTranslation();
  if (targets.length <= 1) return null;
  return (
    <TextField
      fullWidth
      label={t('records.relationshipTargetBlueprint')}
      onChange={(event) => onChange(event.target.value)}
      select
      value={value}
    >
      {targets.map((code) => (
        <MenuItem key={code} value={code}>
          <IconLabel icon={BlueprintIcon}>{blueprintName(code)}</IconLabel>
        </MenuItem>
      ))}
    </TextField>
  );
};

/**
 * A relationship picker's search, with the Explorer beside it as the other
 * way to find targets: Browse opens it in a new tab, which sends its
 * selection back to this window.
 */
export const RelationshipTargetSearchRow = ({
  explorerHref,
  onQueryChange,
  query,
}: {
  explorerHref: string | undefined;
  onQueryChange: (query: string) => void;
  query: string;
}) => {
  const { t } = useTranslation();
  return (
    <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
      <SearchField
        fullWidth
        label={t('records.searchRelationshipOptions')}
        onChange={(event) => onQueryChange(event.target.value)}
        value={query}
      />
      {explorerHref && (
        <Button
          component="a"
          href={explorerHref}
          // The Explorer tab sends its selection back to this window.
          rel="opener"
          startIcon={<ExplorerIcon />}
          sx={{ alignSelf: 'stretch', flexShrink: 0, whiteSpace: 'nowrap' }}
          target="_blank"
          variant="outlined"
        >
          {t('records.chooseInExplorer')}
        </Button>
      )}
    </Stack>
  );
};
