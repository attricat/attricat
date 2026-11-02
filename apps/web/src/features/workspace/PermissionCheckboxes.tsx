import {
  Checkbox,
  FormControlLabel,
  FormGroup,
  type SxProps,
  type Theme,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { permissionDescription } from './permissionDescription';

/** Choosing a role's permissions from those the workspace offers. */
export const PermissionCheckboxes = ({
  onChange,
  permissions,
  selected,
  sx,
}: {
  onChange: (selected: string[]) => void;
  permissions:
    readonly Parameters<typeof permissionDescription>[1][] | undefined;
  selected: readonly string[];
  sx?: SxProps<Theme>;
}) => {
  const { t } = useTranslation();
  return (
    <FormGroup sx={sx}>
      {permissions?.map((permission) => (
        <FormControlLabel
          control={
            <Checkbox
              checked={selected.includes(permission.code)}
              onChange={(event) =>
                onChange(
                  event.target.checked
                    ? [...selected, permission.code]
                    : selected.filter((code) => code !== permission.code),
                )
              }
            />
          }
          key={permission.code}
          label={`${permission.code} — ${permissionDescription(t, permission)}`}
        />
      ))}
    </FormGroup>
  );
};
