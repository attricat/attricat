import { Box, Checkbox, FormControlLabel, Typography } from '@mui/material';
import { useId } from 'react';
import { monoFontFamily } from '../../app/theme';

const permissionGroup = (code: string) => code.split('.')[0] ?? code;

const groupPermissions = <T extends { code: string }>(permissions: T[]) =>
  permissions.reduce((groups, permission) => {
    const group = permissionGroup(permission.code);
    groups.set(group, [...(groups.get(group) ?? []), permission]);
    return groups;
  }, new Map<string, T[]>());

/** Every permission a token can hold, grouped by area, each with a checkbox. */
export const PermissionChecklist = ({
  onToggle,
  permissions,
  selected,
}: {
  onToggle: (code: string, checked: boolean) => void;
  permissions: { code: string; description: string }[];
  selected: readonly string[];
}) => {
  const permissionIdPrefix = useId();
  return (
    <Box
      sx={{
        display: 'grid',
        gap: 5,
        gridTemplateColumns: {
          xs: '1fr',
          sm: 'repeat(2, minmax(0, 1fr))',
        },
      }}
    >
      {[...groupPermissions(permissions)].map(([group, items]) => (
        <Box component="fieldset" key={group} sx={{ border: 0, m: 0, p: 0 }}>
          <Typography
            color="text.secondary"
            component="legend"
            sx={{ fontFamily: monoFontFamily, mb: 1 }}
            variant="overline"
          >
            {group}
          </Typography>
          {items.map((permission) => {
            const idPrefix = `${permissionIdPrefix}-${permission.code}`;
            const labelId = `${idPrefix}-label`;
            const descriptionId = `${idPrefix}-description`;
            return (
              <FormControlLabel
                control={
                  <Checkbox
                    checked={selected.includes(permission.code)}
                    onChange={(event) =>
                      onToggle(permission.code, event.target.checked)
                    }
                    slotProps={{
                      input: {
                        'aria-describedby': descriptionId,
                        'aria-labelledby': labelId,
                      },
                    }}
                  />
                }
                key={permission.code}
                label={
                  <Box sx={{ py: 1 }}>
                    <Typography
                      component="span"
                      id={labelId}
                      sx={{
                        display: 'block',
                        fontFamily: monoFontFamily,
                      }}
                      variant="body2"
                    >
                      {permission.code}
                    </Typography>
                    <Typography
                      color="text.secondary"
                      id={descriptionId}
                      sx={{ display: 'block' }}
                      variant="caption"
                    >
                      {permission.description}
                    </Typography>
                  </Box>
                }
                sx={{
                  alignItems: 'flex-start',
                  display: 'flex',
                  mr: 0,
                }}
              />
            );
          })}
        </Box>
      ))}
    </Box>
  );
};
