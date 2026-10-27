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

const maxTemplateColumns = 3;

export type DefinitionTemplate = {
  definition: string;
  descriptionKey: string;
  labelKey: string;
};

/** Offers valid starting definitions for a new blueprint or attribute. */
export const DefinitionTemplateDialog = ({
  onClose,
  onSelect,
  open,
  selectedIndex,
  templates,
}: {
  onClose: () => void;
  onSelect: (index: number) => void;
  open: boolean;
  selectedIndex: number;
  templates: readonly DefinitionTemplate[];
}) => {
  const { t } = useTranslation();
  return (
    <Dialog fullWidth maxWidth="md" onClose={onClose} open={open}>
      <DialogTitle>{t('definitionEditor.templates.title')}</DialogTitle>
      <DialogContent>
        <Typography color="text.secondary">
          {t('definitionEditor.templates.description')}
        </Typography>
        <Box
          sx={{
            display: 'grid',
            gap: 2,
            gridTemplateColumns: {
              sm: `repeat(${Math.min(templates.length, maxTemplateColumns)}, minmax(0, 1fr))`,
              xs: '1fr',
            },
            mt: 2,
          }}
        >
          {templates.map((template, index) => {
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
                <CardActionArea
                  aria-pressed={selected}
                  onClick={() => onSelect(index)}
                >
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
        <Button onClick={onClose}>
          {t('definitionEditor.templates.dismiss')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};
