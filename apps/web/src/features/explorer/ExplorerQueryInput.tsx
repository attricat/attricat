import {
  Box,
  CircularProgress,
  FormHelperText,
  IconButton,
  InputAdornment,
  Paper,
  Popper,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { CircleQuestionMarkIcon } from 'lucide-react';
import {
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type RefObject,
  type SyntheticEvent,
} from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { compactIconSize, smallIconSize } from '../../components/iconSizes';
import type { Blueprint, BlueprintWithAttributes } from '../records/api';
import { querySuggestionListMaxHeight } from './constants';
import {
  firstQueryError,
  highlightQuery,
  type QueryError,
} from './queryLanguage';
import {
  applyQuerySuggestion,
  querySuggestions,
  type QuerySuggestion,
} from './querySuggestions';
import { useQuerySchema } from './useQuerySchema';
import {
  HighlightedInput,
  QuerySegmentsProvider,
} from './QueryHighlightedInput';
import { QuerySuggestionOption } from './QuerySuggestionOption';
import { errorMessage } from '../../api/errors';

type Props = {
  value: string;
  onChange: (value: string) => void;
  /** Selected blueprint schema; highlighting and completion need it. */
  blueprint?: BlueprintWithAttributes;
  blueprints: Blueprint[];
  onShowSyntax: (anchor: HTMLElement) => void;
  /**
   * Where to render query errors. Rendering them outside the field keeps
   * neighbouring controls in a row from stretching to the helper text.
   */
  errorContainer?: HTMLElement | null;
  /** Receives the input, so other controls can return focus to it. */
  queryInputRef?: RefObject<HTMLInputElement | null>;
};

export const ExplorerQueryInput = ({
  value,
  onChange,
  blueprint,
  blueprints,
  onShowSyntax,
  errorContainer,
  queryInputRef,
}: Props) => {
  const { t } = useTranslation();
  const listboxId = useId();
  const errorId = useId();
  const optionId = (index: number) => `${listboxId}-option-${index}`;
  const [anchor, setAnchor] = useState<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const pendingCursor = useRef<number | null>(null);
  const [cursor, setCursor] = useState(value.length);
  const [open, setOpen] = useState(false);
  const [focused, setFocused] = useState(false);
  const [active, setActive] = useState(-1);
  const { schema, blueprintNames, targetsError, targetsLoading } =
    useQuerySchema(blueprint, blueprints, value);
  const segments = useMemo(
    () => highlightQuery(value, schema),
    [value, schema],
  );
  const error = firstQueryError(segments, value, focused ? cursor : undefined);
  const completion = schema
    ? querySuggestions(
        schema,
        value,
        Math.min(cursor, value.length),
        blueprintNames,
      )
    : undefined;
  const suggestions = completion?.suggestions ?? [];
  const valueHint =
    completion?.context.kind === 'value'
      ? completion.context.selector
      : undefined;
  const loading = Boolean(completion?.loading && targetsLoading);
  const loadFailure =
    completion?.loading && !targetsLoading ? targetsError : undefined;
  const showPopup =
    open &&
    focused &&
    Boolean(suggestions.length || loading || loadFailure || valueHint);
  const activeIndex = active < suggestions.length ? active : -1;

  useLayoutEffect(() => {
    if (pendingCursor.current === null || !inputRef.current) return;
    inputRef.current.setSelectionRange(
      pendingCursor.current,
      pendingCursor.current,
    );
    pendingCursor.current = null;
  });
  useLayoutEffect(() => {
    if (activeIndex < 0) return;
    listRef.current
      ?.querySelector(`#${CSS.escape(optionId(activeIndex))}`)
      ?.scrollIntoView({ block: 'nearest' });
  });

  const trackCursor = (
    event: SyntheticEvent<HTMLInputElement | HTMLTextAreaElement>,
  ) =>
    setCursor(
      event.currentTarget.selectionStart ?? event.currentTarget.value.length,
    );

  const accept = (suggestion: QuerySuggestion) => {
    if (!completion) return;
    const next = applyQuerySuggestion(value, completion.context, suggestion);
    pendingCursor.current = next.cursor;
    onChange(next.query);
    setCursor(next.cursor);
    setActive(-1);
    setOpen(true);
    inputRef.current?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === ' ' && event.ctrlKey) {
      event.preventDefault();
      setOpen(true);
      return;
    }
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      if (!suggestions.length) return;
      event.preventDefault();
      if (!showPopup) {
        setOpen(true);
        setActive(event.key === 'ArrowDown' ? 0 : suggestions.length - 1);
        return;
      }
      // Cycle through the suggestions and back to the typed text (-1).
      const next = activeIndex + (event.key === 'ArrowDown' ? 1 : -1);
      setActive(
        next >= suggestions.length
          ? -1
          : next < -1
            ? suggestions.length - 1
            : next,
      );
      return;
    }
    if (!showPopup) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
      return;
    }
    // Enter submits the search unless a suggestion was chosen with the
    // arrow keys; Tab completes the highlighted or first suggestion.
    const chosen =
      event.key === 'Enter'
        ? suggestions[activeIndex]
        : event.key === 'Tab' && !event.shiftKey
          ? suggestions[Math.max(activeIndex, 0)]
          : undefined;
    if (chosen) {
      event.preventDefault();
      accept(chosen);
    }
  };

  const queryErrorMessage = (queryError: QueryError) =>
    t(`explorer.queryErrors.${queryError.code}`, { ...queryError });

  return (
    <QuerySegmentsProvider segments={segments}>
      <TextField
        error={Boolean(error)}
        fullWidth
        helperText={
          error && !errorContainer ? queryErrorMessage(error) : undefined
        }
        inputRef={(element: HTMLInputElement | null) => {
          inputRef.current = element;
          if (queryInputRef) queryInputRef.current = element;
        }}
        label={t('explorer.query')}
        onBlur={() => {
          setFocused(false);
          setOpen(false);
        }}
        onChange={(event) => {
          onChange(event.target.value);
          trackCursor(event);
          setActive(-1);
          setOpen(true);
        }}
        onFocus={(event) => {
          setFocused(true);
          setOpen(true);
          trackCursor(event);
        }}
        placeholder={t('explorer.searchTerms')}
        ref={setAnchor}
        slotProps={{
          htmlInput: {
            'aria-activedescendant':
              showPopup && activeIndex >= 0 ? optionId(activeIndex) : undefined,
            'aria-autocomplete': 'list',
            ...(error && errorContainer && { 'aria-describedby': errorId }),
            'aria-controls': showPopup ? listboxId : undefined,
            'aria-expanded': showPopup,
            onKeyDown,
            onSelect: trackCursor,
            autoCapitalize: 'off',
            autoComplete: 'off',
            autoCorrect: 'off',
            role: 'combobox',
            spellCheck: false,
          },
          input: {
            inputComponent: HighlightedInput,
            endAdornment: (
              <InputAdornment position="end">
                <Tooltip title={t('explorer.searchSyntax')}>
                  <IconButton
                    aria-label={t('explorer.searchSyntax')}
                    onClick={(event) => onShowSyntax(event.currentTarget)}
                    size="small"
                    type="button"
                  >
                    <CircleQuestionMarkIcon size={smallIconSize} />
                  </IconButton>
                </Tooltip>
              </InputAdornment>
            ),
          },
          // Keep the label raised so the "Search anything" prompt always shows.
          inputLabel: { shrink: true },
        }}
        sx={{
          '& input::placeholder': { color: 'text.disabled', opacity: 1 },
          '& input': { caretColor: (theme) => theme.palette.text.primary },
        }}
        value={value}
      />
      {error &&
        errorContainer &&
        createPortal(
          <FormHelperText error id={errorId} sx={{ mx: 3.5 }}>
            {queryErrorMessage(error)}
          </FormHelperText>,
          errorContainer,
        )}
      <Popper
        anchorEl={anchor}
        open={showPopup}
        placement="bottom-start"
        sx={(theme) => ({
          width: anchor?.clientWidth,
          zIndex: theme.zIndex.modal,
        })}
      >
        <Paper
          elevation={8}
          // Keep focus in the input while clicking suggestions.
          onMouseDown={(event) => event.preventDefault()}
          sx={{ mt: 1, overflow: 'hidden' }}
        >
          {valueHint !== undefined && (
            <Typography
              color="text.secondary"
              sx={{ px: 3, pt: 2, pb: suggestions.length ? 1 : 2 }}
              variant="body2"
            >
              {t('explorer.querySuggestions.valueHint', { field: valueHint })}
            </Typography>
          )}
          {loading && (
            <Stack
              direction="row"
              spacing={2}
              sx={{ alignItems: 'center', px: 3, py: 2 }}
            >
              <CircularProgress size={compactIconSize} />
              <Typography color="text.secondary" variant="body2">
                {t('explorer.loadingRelationshipFields')}
              </Typography>
            </Stack>
          )}
          {loadFailure && (
            <Typography
              color="error"
              role="alert"
              sx={{ px: 3, py: 2 }}
              variant="body2"
            >
              {errorMessage(loadFailure)}
            </Typography>
          )}
          {suggestions.length > 0 && (
            <Box
              aria-label={t('explorer.querySuggestions.label')}
              component="ul"
              id={listboxId}
              ref={listRef}
              role="listbox"
              sx={{
                listStyle: 'none',
                m: 0,
                maxHeight: querySuggestionListMaxHeight,
                overflowY: 'auto',
                p: 1,
              }}
            >
              {suggestions.map((suggestion, index) => {
                return (
                  <QuerySuggestionOption
                    active={index === activeIndex}
                    id={optionId(index)}
                    key={`${suggestion.kind}:${suggestion.code}`}
                    onHover={() => {
                      if (index !== activeIndex) setActive(index);
                    }}
                    onSelect={() => accept(suggestion)}
                    suggestion={suggestion}
                  />
                );
              })}
            </Box>
          )}
          {suggestions.length > 0 && (
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
              {t('explorer.querySuggestions.keyboardHint')}
            </Typography>
          )}
        </Paper>
      </Popper>
    </QuerySegmentsProvider>
  );
};
