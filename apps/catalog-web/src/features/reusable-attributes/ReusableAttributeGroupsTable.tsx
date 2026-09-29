import {
  Paper,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { ReusableAttributeGroup } from './schemas';
import { GROUPS_TABLE_COLUMN_COUNT } from './constants';

export const ReusableAttributeGroupsTable = ({
  groups,
}: {
  groups: ReusableAttributeGroup[];
}) => {
  const { t } = useTranslation();
  return (
    <Paper sx={{ mt: 2 }}>
      <Table size="small">
        <TableHead>
          <TableRow>
            <TableCell>{t('reusableAttributes.columns.group')}</TableCell>
            <TableCell>{t('reusableAttributes.columns.position')}</TableCell>
            <TableCell>
              {t('reusableAttributes.columns.publishedRevisions')}
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
              <TableCell>
                {group.reusable_attribute_revision_ids.length}
              </TableCell>
            </TableRow>
          ))}
          {!groups.length && (
            <TableRow>
              <TableCell colSpan={GROUPS_TABLE_COLUMN_COUNT}>
                {t('reusableAttributes.groupsEmpty')}
              </TableCell>
            </TableRow>
          )}
        </TableBody>
      </Table>
    </Paper>
  );
};
