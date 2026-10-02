import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  IconButton,
  Stack,
  Tooltip,
} from '@mui/material';
import { ArrowLeftIcon, ArrowRightIcon, Trash2Icon } from 'lucide-react';
import { useId, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { ImageGallery, type GalleryFile } from './ImageGallery';

const GalleryAction = ({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled: boolean;
  onClick: () => void;
  children: ReactNode;
}) => (
  <Tooltip title={label}>
    {/* Disabled buttons emit no events; the span keeps the tooltip working. */}
    <span>
      <IconButton aria-label={label} disabled={disabled} onClick={onClick}>
        {children}
      </IconButton>
    </span>
  </Tooltip>
);

export const ImageGalleryEditor = ({
  files,
  disabled,
  ordered,
  onChange,
}: {
  files: readonly GalleryFile[];
  disabled: boolean;
  ordered: boolean;
  onChange: (fileIds: string[]) => Promise<boolean>;
}) => {
  const { t } = useTranslation();
  const [removeId, setRemoveId] = useState<string | null>(null);
  const titleId = useId();
  const descriptionId = useId();
  const removedFile = files.find((file) => file.id === removeId);
  const move = (index: number, offset: number) => {
    if (disabled || !ordered) return;
    const ids = files.map((file) => file.id);
    [ids[index], ids[index + offset]] = [ids[index + offset], ids[index]];
    void onChange(ids);
  };
  return (
    <>
      <ImageGallery
        files={files}
        renderActions={(file, index) => (
          <Stack direction="row">
            {ordered && (
              <>
                <GalleryAction
                  label={t('files.moveEarlier', { filename: file.filename })}
                  disabled={disabled || index === 0}
                  onClick={() => move(index, -1)}
                >
                  <ArrowLeftIcon />
                </GalleryAction>
                <GalleryAction
                  label={t('files.moveLater', { filename: file.filename })}
                  disabled={disabled || index === files.length - 1}
                  onClick={() => move(index, 1)}
                >
                  <ArrowRightIcon />
                </GalleryAction>
              </>
            )}
            <GalleryAction
              label={t('files.removeImage', { filename: file.filename })}
              disabled={disabled}
              onClick={() => {
                if (!disabled) setRemoveId(file.id);
              }}
            >
              <Trash2Icon />
            </GalleryAction>
          </Stack>
        )}
      />
      <Dialog
        open={Boolean(removedFile)}
        onClose={() => setRemoveId(null)}
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
      >
        <DialogTitle id={titleId}>
          {t('files.removeImage', { filename: removedFile?.filename })}
        </DialogTitle>
        <DialogContent>
          <DialogContentText id={descriptionId}>
            {t('files.removeImageExplanation')}
          </DialogContentText>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setRemoveId(null)}>
            {t('common.cancel')}
          </Button>
          <Button
            color="error"
            disabled={disabled}
            onClick={async () => {
              if (disabled) return;
              // Close on failure too, so inline server errors are not hidden by the dialog.
              await onChange(
                files
                  .filter((file) => file.id !== removeId)
                  .map((file) => file.id),
              );
              setRemoveId(null);
            }}
          >
            {t('files.confirmRemove')}
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
};
