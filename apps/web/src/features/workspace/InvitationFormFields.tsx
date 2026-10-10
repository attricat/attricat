import { MenuItem, TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';

export const RoleSelectField = ({
  onChange,
  roles,
  value,
}: {
  onChange: (value: string) => void;
  roles?: { code: string; id: string }[];
  value: string;
}) => {
  const { t } = useTranslation();
  return (
    <TextField
      required
      label={t('workspace.role')}
      onChange={(event) => onChange(event.target.value)}
      select
      value={value}
    >
      {roles?.map((role) => (
        <MenuItem key={role.id} value={role.id}>
          {role.code}
        </MenuItem>
      ))}
    </TextField>
  );
};

export const ExpiryField = ({
  onChange,
  value,
}: {
  onChange: (value: string) => void;
  value: string;
}) => {
  const { t } = useTranslation();
  return (
    <TextField
      required
      label={t('workspace.expiresAt')}
      onChange={(event) => onChange(event.target.value)}
      slotProps={{ inputLabel: { shrink: true } }}
      type="datetime-local"
      value={value}
    />
  );
};
