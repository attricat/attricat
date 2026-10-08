import { Accessibility, type Draggable } from '@dnd-kit/dom';
import { move as moveDragged } from '@dnd-kit/helpers';
import { DragDropProvider } from '@dnd-kit/react';
import { isSortable, useSortable } from '@dnd-kit/react/sortable';
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
import {
  ArrowLeftIcon,
  ArrowRightIcon,
  GripVerticalIcon,
  Trash2Icon,
} from 'lucide-react';
import {
  createContext,
  useContext,
  useId,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import { useTranslation } from 'react-i18next';
import { ImageGallery, type GalleryFile } from './ImageGallery';

// `Accessibility.configure` is untyped, so take its options from the plugin.
type AccessibilityOptions = NonNullable<
  ConstructorParameters<typeof Accessibility>[1]
>;

// The handle sits among the tile's actions, away from the hook that owns it.
const DragHandleContext = createContext<{
  dragging: boolean;
  handleRef: (element: Element | null) => void;
} | null>(null);

const SortableTile = ({
  children,
  disabled,
  id,
  index,
}: {
  children: ReactNode;
  disabled: boolean;
  id: string;
  index: number;
}) => {
  const { handleRef, isDragSource, ref } = useSortable({ disabled, id, index });
  const handle = useMemo(
    () => ({ dragging: isDragSource, handleRef }),
    [handleRef, isDragSource],
  );
  return (
    <Stack
      ref={ref}
      spacing={1}
      sx={{
        bgcolor: 'background.paper',
        borderRadius: 1,
        boxShadow: isDragSource ? 3 : 0,
        minWidth: 0,
        // Shadows barely show on dark paper, so outline the lifted tile too.
        outline: (theme) =>
          isDragSource ? `1px solid ${theme.palette.divider}` : 'none',
        zIndex: isDragSource ? 1 : undefined,
      }}
    >
      <DragHandleContext.Provider value={handle}>
        {children}
      </DragHandleContext.Provider>
    </Stack>
  );
};

const DragHandle = ({
  disabled,
  label,
}: {
  disabled: boolean;
  label: string;
}) => {
  const handle = useContext(DragHandleContext);
  return (
    <IconButton
      aria-label={label}
      disabled={disabled}
      ref={handle?.handleRef}
      sx={{
        cursor: handle?.dragging ? 'grabbing' : 'grab',
        touchAction: 'none',
      }}
    >
      <GripVerticalIcon />
    </IconButton>
  );
};

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
  // Shown until the save settles, so a dropped tile does not jump back.
  const [pendingOrder, setPendingOrder] = useState<string[] | null>(null);
  const titleId = useId();
  const descriptionId = useId();
  const removedFile = files.find((file) => file.id === removeId);
  const shown = useMemo(() => {
    if (!pendingOrder) return files;
    const byId = new Map(files.map((file) => [file.id, file]));
    const sorted = pendingOrder.flatMap((id) => byId.get(id) ?? []);
    return sorted.length === files.length ? sorted : files;
  }, [files, pendingOrder]);
  const reorder = async (ids: string[]) => {
    setPendingOrder(ids);
    try {
      await onChange(ids);
    } finally {
      setPendingOrder(null);
    }
  };
  const move = (index: number, offset: number) => {
    if (disabled || !ordered) return;
    const ids = shown.map((file) => file.id);
    [ids[index], ids[index + offset]] = [ids[index + offset], ids[index]];
    void reorder(ids);
  };
  // dnd-kit announces IDs in English by default; announce file names and
  // positions in the user's language instead.
  const count = shown.length;
  const plugins = useMemo(() => {
    const names = new Map(shown.map((file) => [file.id, file.filename]));
    const announce = (key: string, source: Draggable | null) =>
      source
        ? t(key, {
            filename: names.get(String(source.id)) ?? String(source.id),
            position: isSortable(source) ? source.index + 1 : undefined,
            count,
          })
        : undefined;
    const options: AccessibilityOptions = {
      announcements: {
        dragstart: ({ operation: { source } }) =>
          announce('files.imageDragStart', source),
        dragover: ({ operation: { source } }) =>
          announce('files.imageDragOver', source),
        dragend: ({ operation: { source }, canceled }) =>
          announce(
            canceled ? 'files.imageDragCancel' : 'files.imageDragEnd',
            source,
          ),
      },
      screenReaderInstructions: {
        draggable: t('files.imageDragInstructions'),
      },
    };
    const accessibility = Accessibility.configure(options);
    return <T,>(defaults: T[]) =>
      defaults.map((plugin) =>
        plugin === Accessibility ? accessibility : plugin,
      );
  }, [count, shown, t]);
  const gallery = (
    <ImageGallery
      files={shown}
      renderItem={
        ordered
          ? ({ children, file, index }) => (
              <SortableTile
                disabled={disabled}
                id={file.id}
                index={index}
                key={file.id}
              >
                {children}
              </SortableTile>
            )
          : undefined
      }
      renderActions={(file, index) => (
        <Stack direction="row">
          {ordered && (
            <>
              <DragHandle
                disabled={disabled}
                label={t('files.dragImage', { filename: file.filename })}
              />
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
  );
  return (
    <>
      {ordered ? (
        <DragDropProvider
          onDragEnd={(event) => {
            if (event.canceled || disabled) return;
            const ids = shown.map((file) => file.id);
            const next = moveDragged(ids, event);
            if (next !== ids) void reorder(next);
          }}
          plugins={plugins}
        >
          {gallery}
        </DragDropProvider>
      ) : (
        gallery
      )}
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
