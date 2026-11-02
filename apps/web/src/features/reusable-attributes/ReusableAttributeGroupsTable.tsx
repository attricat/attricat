import {
  Button,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { PencilIcon, Trash2Icon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ReusableAttributeGroup } from './schemas';
import { GROUPS_TABLE_COLUMN_COUNT } from './constants';
import { EmptyState } from '../../components/EmptyState';
import { ReusableAttributeIcon } from '../../components/systemIcons';

export const ReusableAttributeGroupsTable = ({
  canEdit,
  groups,
  onDelete,
  onEdit,
}: {
  /** The caller may edit and delete reusable attribute groups. */
  canEdit: boolean;
  groups: ReusableAttributeGroup[];
  onDelete: (group: ReusableAttributeGroup) => void;
  onEdit: (group: ReusableAttributeGroup) => void;
}) => {
  const { t } = useTranslation();
  return (
    <TableContainer component={Paper} sx={{ mt: 2 }}>
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>{t('reusableAttributes.columns.group')}</TableCell>
            <TableCell>{t('reusableAttributes.columns.position')}</TableCell>
            <TableCell>{t('reusableAttributes.columns.attributes')}</TableCell>
            <TableCell align="right">
              {t('reusableAttributes.columns.actions')}
            </TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {groups.map((group) => (
            <TableRow key={group.id}>
              <TableCell>
                <Typography>{group.name}</Typography>
                <Typography color="text.secondary" variant="caption">
                  {group.code}
                </Typography>
              </TableCell>
              <TableCell>{group.position}</TableCell>
              <TableCell>{group.reusable_attribute_codes.length}</TableCell>
              <TableCell align="right">
                {canEdit && (
                  <Stack
                    direction="row"
                    spacing={1}
                    sx={{ justifyContent: 'flex-end' }}
                  >
                    <Button
                      aria-label={t('reusableAttributes.editNamedGroup', {
                        group: group.name,
                      })}
                      onClick={() => onEdit(group)}
                      size="small"
                      startIcon={<PencilIcon />}
                    >
                      {t('reusableAttributes.edit')}
                    </Button>
                    <Button
                      aria-label={t('reusableAttributes.deleteNamedGroup', {
                        group: group.name,
                      })}
                      color="error"
                      onClick={() => onDelete(group)}
                      size="small"
                      startIcon={<Trash2Icon />}
                    >
                      {t('reusableAttributes.deleteGroup')}
                    </Button>
                  </Stack>
                )}
              </TableCell>
            </TableRow>
          ))}
          {!groups.length && (
            <TableRow>
              <TableCell colSpan={GROUPS_TABLE_COLUMN_COUNT}>
                <EmptyState
                  icon={ReusableAttributeIcon}
                  title={t('reusableAttributes.groupsEmpty')}
                />
              </TableCell>
            </TableRow>
          )}
        </TableBody>
      </Table>
    </TableContainer>
  );
};
