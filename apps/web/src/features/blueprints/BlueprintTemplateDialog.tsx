import {
  Box,
  Button,
  Card,
  CardActionArea,
  CardContent,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { blueprintTemplates } from './blueprintEditorUtils';

export const BlueprintTemplateDialog = ({
  onClose,
  onSelect,
  open,
  selectedIndex,
}: {
  onClose: () => void;
  onSelect: (index: number) => void;
  open: boolean;
  selectedIndex: number;
}) => {
  const { t } = useTranslation();
  return (
    <Dialog fullWidth maxWidth="md" onClose={onClose} open={open}>
      <DialogTitle>{t('blueprints.startFromExample')}</DialogTitle>
      <DialogContent>
        <Typography color="text.secondary">
          {t('blueprints.templateDescription')}
        </Typography>
        <Box
          sx={{
            display: 'grid',
            gap: 2,
            gridTemplateColumns: {
              sm: `repeat(${blueprintTemplates.length}, minmax(0, 1fr))`,
              xs: '1fr',
            },
            mt: 2,
          }}
        >
          {blueprintTemplates.map((template, index) => {
            const selected = selectedIndex === index;
            return (
              <Card
                key={template.labelKey}
                sx={{
                  border: selected ? 2 : 1,
                  borderColor: selected ? 'primary.main' : 'divider',
                }}
                variant="outlined"
              >
                <CardActionArea onClick={() => onSelect(index)}>
                  <CardContent>
                    <Typography component="h3" variant="h6">
                      {t(template.labelKey)}
                    </Typography>
                    <Typography color="text.secondary" sx={{ mt: 1 }}>
                      {t(template.descriptionKey)}
                    </Typography>
                  </CardContent>
                </CardActionArea>
              </Card>
            );
          })}
        </Box>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>{t('blueprints.dismiss')}</Button>
      </DialogActions>
    </Dialog>
  );
};
