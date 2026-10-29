import {
  Box,
  Paper,
  Popper,
  Stack,
  TextField,
  Typography,
  type TextFieldProps,
} from '@mui/material';
import {
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type SyntheticEvent,
} from 'react';
import { useTranslation } from 'react-i18next';
import { UserAvatar } from '../../components/UserAvatar';
import { userAvatarSizes } from '../../components/userAvatars';
import {
  activeMentionQuery,
  insertMention,
  mentionSuggestions,
} from '../markdown/mentions';
import { userLabel } from '../principals/principal';
import type { DirectoryUser } from '../principals/schemas';
import { usePrincipalDirectory } from '../principals/usePrincipalDirectory';
import {
  maxMentionSuggestions,
  mentionSuggestionListMaxHeight,
} from './constants';

type Props = Omit<
  TextFieldProps,
  'value' | 'onChange' | 'inputRef' | 'multiline'
> & {
  value: string;
  onValueChange: (value: string) => void;
};

/**
 * A multiline text field that suggests workspace members after `@` and
 * inserts the chosen one as a mention.
 */
export const MentionTextField = ({
  value,
  onValueChange,
  onBlur,
  ...props
}: Props) => {
  const { t } = useTranslation();
  const listboxId = useId();
  const optionId = (index: number) => `${listboxId}-option-${index}`;
  const [anchor, setAnchor] = useState<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const pendingCursor = useRef<number | null>(null);
  const [cursor, setCursor] = useState(value.length);
  const [focused, setFocused] = useState(false);
  const [dismissed, setDismissed] = useState(false);
  const [active, setActive] = useState(0);
  // The directory loads once someone starts writing, not for every reader.
  const [used, setUsed] = useState(false);
  const directory = usePrincipalDirectory(used);
  const mention = focused
    ? activeMentionQuery(value, Math.min(cursor, value.length))
    : undefined;
  const suggestions = mention
    ? mentionSuggestions(
        directory.data?.users ?? [],
        mention.query,
        maxMentionSuggestions,
      )
    : [];
  const open = Boolean(mention) && !dismissed && suggestions.length > 0;
  const activeIndex = active < suggestions.length ? active : 0;

  useLayoutEffect(() => {
    if (pendingCursor.current === null || !inputRef.current) return;
    inputRef.current.setSelectionRange(
      pendingCursor.current,
      pendingCursor.current,
    );
    pendingCursor.current = null;
  });
  useLayoutEffect(() => {
    if (!open) return;
    listRef.current
      ?.querySelector(`#${CSS.escape(optionId(activeIndex))}`)
      ?.scrollIntoView({ block: 'nearest' });
  });

  const trackCursor = (
    event: SyntheticEvent<HTMLInputElement | HTMLTextAreaElement>,
  ) => {
    const next =
      event.currentTarget.selectionStart ?? event.currentTarget.value.length;
    if (next !== cursor) {
      setCursor(next);
      setActive(0);
    }
  };

  const accept = (user: DirectoryUser) => {
    if (!mention) return;
    const next = insertMention(value, mention, user);
    pendingCursor.current = next.cursor;
    onValueChange(next.text);
    setCursor(next.cursor);
    setActive(0);
    inputRef.current?.focus();
  };

  const onKeyDown = (
    event: KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>,
  ) => {
    if (!open) return;
    switch (event.key) {
      case 'ArrowDown':
      case 'ArrowUp': {
        event.preventDefault();
        const step = event.key === 'ArrowDown' ? 1 : -1;
        setActive(
          (activeIndex + step + suggestions.length) % suggestions.length,
        );
        return;
      }
      case 'Enter':
      case 'Tab':
        if (event.shiftKey) return;
        event.preventDefault();
        accept(suggestions[activeIndex]);
        return;
      case 'Escape':
        event.preventDefault();
        event.stopPropagation();
        setDismissed(true);
        return;
    }
  };

  return (
    <>
      <TextField
        {...props}
        multiline
        value={value}
        ref={setAnchor}
        inputRef={inputRef}
        onChange={(event) => {
          onValueChange(event.target.value);
          trackCursor(event);
          setDismissed(false);
        }}
        onFocus={(event) => {
          setFocused(true);
          setUsed(true);
          trackCursor(event);
        }}
        onBlur={(event) => {
          setFocused(false);
          setDismissed(false);
          onBlur?.(event);
        }}
        slotProps={{
          htmlInput: {
            'aria-activedescendant': open ? optionId(activeIndex) : undefined,
            'aria-autocomplete': 'list',
            'aria-controls': open ? listboxId : undefined,
            onKeyDown,
            onSelect: trackCursor,
          },
        }}
      />
      <Popper
        anchorEl={anchor}
        open={open}
        placement="bottom-start"
        sx={(theme) => ({ zIndex: theme.zIndex.modal })}
      >
        <Paper
          elevation={8}
          // Keep focus in the field while clicking suggestions.
          onMouseDown={(event) => event.preventDefault()}
          sx={{ mt: 1, overflow: 'hidden' }}
        >
          <Box
            aria-label={t('comments.mentionSuggestions')}
            component="ul"
            id={listboxId}
            ref={listRef}
            role="listbox"
            sx={{
              listStyle: 'none',
              m: 0,
              maxHeight: mentionSuggestionListMaxHeight,
              overflowY: 'auto',
              p: 1,
            }}
          >
            {suggestions.map((user, index) => (
              <Stack
                aria-selected={index === activeIndex}
                component="li"
                direction="row"
                id={optionId(index)}
                key={user.id}
                onClick={() => accept(user)}
                onMouseMove={() => {
                  if (index !== activeIndex) setActive(index);
                }}
                role="option"
                spacing={2}
                sx={{
                  alignItems: 'center',
                  borderRadius: 1,
                  cursor: 'pointer',
                  px: 2,
                  py: 1,
                  ...(index === activeIndex && { bgcolor: 'action.selected' }),
                }}
              >
                <UserAvatar
                  name={userLabel(user)}
                  size={userAvatarSizes.message}
                />
                <Box sx={{ minWidth: 0 }}>
                  <Typography
                    component="span"
                    sx={{ display: 'block', fontWeight: 600 }}
                    variant="body2"
                  >
                    {userLabel(user)}
                  </Typography>
                  {user.display_name?.trim() && (
                    <Typography
                      color="text.secondary"
                      component="span"
                      sx={{ display: 'block', overflowWrap: 'anywhere' }}
                      variant="caption"
                    >
                      {user.email}
                    </Typography>
                  )}
                </Box>
              </Stack>
            ))}
          </Box>
          <Typography
            color="text.secondary"
            component="div"
            sx={{
              borderColor: 'divider',
              borderTop: 1,
              display: { xs: 'none', sm: 'block' },
              px: 3,
              py: 1.5,
            }}
            variant="caption"
          >
            {t('comments.mentionKeyboardHint')}
          </Typography>
        </Paper>
      </Popper>
    </>
  );
};
