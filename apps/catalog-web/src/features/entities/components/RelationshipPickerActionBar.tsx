import { Button, Paper } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { RelationshipPickerIcon } from '../../../components/systemIcons';
import { relationshipPickerMessageType } from './useRecentlyPreviewedEntities';

export const RelationshipPickerActionBar = ({
  entityId,
  pickerToken,
}: {
  entityId: string;
  pickerToken: string;
}) => {
  const { t } = useTranslation();
  if (!window.opener) return null;

  const selectAndClose = () => {
    window.opener.postMessage(
      {
        type: relationshipPickerMessageType,
        token: pickerToken,
        entityId,
      },
      window.location.origin,
    );
    window.close();
  };

  return (
    <Paper
      aria-label={t('entities.relationshipPickerAction')}
      component="aside"
      sx={{
        alignItems: 'center',
        bgcolor: 'action.hover',
        borderColor: 'primary.main',
        display: 'flex',
        justifyContent: 'center',
        mb: 2,
        p: { xs: 1.5, sm: 2 },
      }}
      variant="outlined"
    >
      <Button
        onClick={selectAndClose}
        size="large"
        startIcon={<RelationshipPickerIcon />}
        sx={{ minWidth: { sm: 300 } }}
        variant="contained"
      >
        {t('entities.selectThisEntityAndClose')}
      </Button>
    </Paper>
  );
};
