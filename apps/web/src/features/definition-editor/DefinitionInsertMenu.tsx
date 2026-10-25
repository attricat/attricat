import {
  Button,
  Divider,
  ListItemIcon,
  ListItemText,
  ListSubheader,
  Menu,
  MenuItem,
} from '@mui/material';
import { ChevronLeftIcon, ChevronRightIcon, PlusIcon } from 'lucide-react';
import { useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import type { DefinitionKind } from './constants';
import {
  definitionSnippets,
  snippetIsPresent,
  type DefinitionSnippet,
} from './definitionSnippets';
import { scanToml } from './tomlOutline';

const insertMenuMaxHeight = 480;
const insertMenuWidth = 340;

/**
 * Inserts ready-made definition pieces in two steps: first the kind of piece,
 * then the piece itself, each with a short description.
 */
export const DefinitionInsertMenu = ({
  definition,
  disabled,
  kind,
  onInsert,
}: {
  definition: string;
  disabled: boolean;
  kind: DefinitionKind;
  onInsert: (snippet: DefinitionSnippet) => void;
}) => {
  const { t } = useTranslation();
  const menuId = useId();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const [group, setGroup] = useState<string>();
  const snippets = definitionSnippets[kind];
  const groups = [...new Set(snippets.map((snippet) => snippet.group))];
  const open = anchor !== null;
  const listRef = useRef<HTMLUListElement>(null);
  // Moving between steps replaces the items, so focus the new first one.
  useEffect(() => {
    if (!open) return;
    listRef.current
      ?.querySelector<HTMLElement>(
        '[role="menuitem"]:not([aria-disabled="true"])',
      )
      ?.focus();
  }, [group, open]);
  const close = () => {
    setAnchor(null);
    setGroup(undefined);
  };
  // Scanned once per render of the open menu, not once per snippet.
  const outline = open ? scanToml(definition) : undefined;
  const available = (snippet: DefinitionSnippet) =>
    !outline || !snippetIsPresent(outline, snippet);

  const groupItems = () =>
    groups.map((name) => {
      const inGroup = snippets.filter((snippet) => snippet.group === name);
      return (
        <MenuItem key={name} onClick={() => setGroup(name)}>
          <ListItemText
            primary={t(`definitionEditor.snippetGroups.${name}`)}
            secondary={t('definitionEditor.insert.available', {
              available: inGroup.filter(available).length,
              total: inGroup.length,
            })}
          />
          <ChevronRightIcon aria-hidden size={compactIconSize} />
        </MenuItem>
      );
    });

  const snippetItems = (name: string) => [
    <MenuItem key="back" onClick={() => setGroup(undefined)}>
      <ListItemIcon>
        <ChevronLeftIcon aria-hidden size={compactIconSize} />
      </ListItemIcon>
      <ListItemText primary={t('definitionEditor.insert.back')} />
    </MenuItem>,
    <Divider key="divider" />,
    <ListSubheader key="group">
      {t(`definitionEditor.snippetGroups.${name}`)}
    </ListSubheader>,
    ...snippets
      .filter((snippet) => snippet.group === name)
      .map((snippet) => {
        const present = !available(snippet);
        return (
          <MenuItem
            disabled={present}
            key={snippet.id}
            onClick={() => {
              close();
              onInsert(snippet);
            }}
          >
            <ListItemText
              primary={t(`definitionEditor.snippets.${snippet.id}`)}
              secondary={
                present
                  ? t('definitionEditor.insert.alreadyDefined')
                  : t(`definitionEditor.snippetDescriptions.${snippet.id}`)
              }
              slotProps={{ secondary: { sx: { whiteSpace: 'normal' } } }}
            />
          </MenuItem>
        );
      }),
  ];

  return (
    <>
      <Button
        aria-controls={open ? menuId : undefined}
        aria-expanded={open ? true : undefined}
        aria-haspopup="menu"
        disabled={disabled}
        onClick={(event) => setAnchor(event.currentTarget)}
        startIcon={<PlusIcon aria-hidden size={compactIconSize} />}
      >
        {t('definitionEditor.insert.button')}
      </Button>
      <Menu
        anchorEl={anchor}
        id={menuId}
        onClose={close}
        open={open}
        slotProps={{
          list: { ref: listRef },
          paper: {
            sx: { maxHeight: insertMenuMaxHeight, width: insertMenuWidth },
          },
        }}
      >
        {group === undefined ? groupItems() : snippetItems(group)}
      </Menu>
    </>
  );
};
