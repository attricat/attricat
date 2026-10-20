import {
  Box,
  FormControl,
  FormHelperText,
  FormLabel,
  IconButton,
  ListItemText,
  MenuItem,
  Popover,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { useId, useRef, useState, type ChangeEvent } from 'react';
import { monoFontFamily } from '../../../app/theme';
import { useTranslation } from 'react-i18next';
import { attributeLabel } from '../../records/recordDisplay';
import {
  statusCodeLabel,
  statusOptionLabel,
  statusTransitionDenial,
  type StatusConfiguration,
} from '../../records/status';
import { useStatusTransitionDenialText } from '../../records/useStatusTransitionDenialText';
import { MarkdownEditor } from '../../markdown/MarkdownEditor';
import type { ValueEditorProps } from '../components/componentTypes';
import type { StatusTransitionAccess } from '../../records/api';
import {
  COLOR_PICKER_SEED,
  COLOR_POPOVER_WIDTH,
  COLOR_SWATCH_SIZE,
  parseColor,
  validateColor,
} from './color';
import { emailHref, validateEmail } from './email';
import { phoneHref } from './phone';
import { CopyableText } from './CopyableText';
import { TextControlEditor } from './TextControl';
import { safeUrl, validateUrl } from './url';
import {
  ExternalLinkIcon,
  MailIcon,
  PencilIcon,
  PhoneIcon,
} from 'lucide-react';
import { compactIconSize, smallIconSize } from '../../../components/iconSizes';
import { FieldHint } from '../../../components/FieldHint';

export const ColorEditor = ({
  attribute,
  value,
  disabled,
  required,
  error,
  helperText,
  onChange,
}: ValueEditorProps) => {
  const { t } = useTranslation();
  const helpId = useId();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const hexInput = useRef<HTMLInputElement>(null);
  const label = attributeLabel(attribute);
  const problem = error ?? validateColor(value);
  const help = problem ?? (helperText && <FieldHint>{helperText}</FieldHint>);
  const color = parseColor(value);
  const change = (next: string) => {
    if (!disabled) onChange(next);
  };
  return (
    // The picker is the control; typing a hex value is tucked into a
    // popover for the rare exact adjustment.
    <FormControl
      aria-describedby={help ? helpId : undefined}
      component="fieldset"
      disabled={disabled}
      error={Boolean(problem)}
      fullWidth
      required={required}
    >
      <FormLabel component="legend" sx={{ mb: 0.5, p: 0 }}>
        {label}
      </FormLabel>
      <Stack direction="row" spacing={2} sx={{ alignItems: 'center' }}>
        <Box
          sx={{
            borderRadius: 1,
            flexShrink: 0,
            height: COLOR_SWATCH_SIZE,
            position: 'relative',
            width: COLOR_SWATCH_SIZE,
            '&:focus-within': {
              outline: '2px solid',
              outlineColor: 'primary.main',
              outlineOffset: 2,
            },
          }}
        >
          {/* The native picker sits invisibly over the swatch it sets. */}
          <Box
            aria-hidden
            sx={{
              border: 1,
              borderColor: 'divider',
              borderRadius: 'inherit',
              height: '100%',
              opacity: disabled ? 0.5 : 1,
              // An unset colour reads as empty, not as the picker's seed.
              ...(color
                ? { bgcolor: color }
                : {
                    background: (theme) =>
                      `linear-gradient(to top right, transparent calc(50% - 1px), ${theme.palette.text.disabled} 50%, transparent calc(50% + 1px))`,
                  }),
            }}
          />
          <Box
            component="input"
            type="color"
            value={color ?? COLOR_PICKER_SEED}
            disabled={disabled}
            aria-label={t('views.colorPickerFor', { attribute: label })}
            aria-describedby={help ? helpId : undefined}
            onChange={(event: ChangeEvent<HTMLInputElement>) =>
              change(event.target.value)
            }
            sx={{
              cursor: disabled ? 'default' : 'pointer',
              height: '100%',
              inset: 0,
              m: 0,
              opacity: 0,
              p: 0,
              position: 'absolute',
              width: '100%',
            }}
          />
        </Box>
        {value ? (
          <CopyableText value={value} sx={{ fontFamily: monoFontFamily }} />
        ) : (
          <Typography color="text.secondary" variant="body2">
            {t('views.notSet')}
          </Typography>
        )}
        <Tooltip title={t('views.colorEditValue')}>
          <span>
            <IconButton
              aria-label={t('views.colorEditValueFor', { attribute: label })}
              disabled={disabled}
              onClick={(event) => setAnchor(event.currentTarget)}
            >
              <PencilIcon size={compactIconSize} />
            </IconButton>
          </span>
        </Tooltip>
      </Stack>
      {help && <FormHelperText id={helpId}>{help}</FormHelperText>}
      <Popover
        anchorEl={anchor}
        anchorOrigin={{ horizontal: 'left', vertical: 'bottom' }}
        onClose={() => setAnchor(null)}
        open={Boolean(anchor)}
        slotProps={{
          transition: { onEntered: () => hexInput.current?.focus() },
        }}
      >
        <Box sx={{ p: 2, width: COLOR_POPOVER_WIDTH }}>
          <TextField
            inputRef={hexInput}
            fullWidth
            label={label}
            value={value}
            disabled={disabled}
            error={Boolean(problem)}
            helperText={problem ?? t('views.colorFormat')}
            onChange={(event) => change(event.target.value)}
            onKeyDown={(event) => {
              if (event.key !== 'Enter') return;
              // Focus returns to the edit button as the popover closes; without
              // this, the same key press would click it open again.
              event.preventDefault();
              setAnchor(null);
            }}
            slotProps={{
              htmlInput: {
                spellCheck: false,
                autoCapitalize: 'none',
                style: { fontFamily: monoFontFamily },
              },
            }}
          />
        </Box>
      </Popover>
    </FormControl>
  );
};

export const EmailEditor = (props: ValueEditorProps) => {
  const { t } = useTranslation();
  return (
    <TextControlEditor
      {...props}
      type="email"
      validate={validateEmail}
      link={{
        href: emailHref,
        label: (address) => t('views.emailTo', { address }),
        icon: <MailIcon size={smallIconSize} />,
      }}
    />
  );
};

export const UrlEditor = (props: ValueEditorProps) => {
  const { t } = useTranslation();
  return (
    <TextControlEditor
      {...props}
      type="url"
      validate={validateUrl}
      link={{
        href: safeUrl,
        label: (url) => t('views.urlOpenNewTab', { url }),
        icon: <ExternalLinkIcon size={smallIconSize} />,
        newTab: true,
      }}
    />
  );
};

export const PhoneEditor = (props: ValueEditorProps) => {
  const { t } = useTranslation();
  return (
    <TextControlEditor
      {...props}
      type="tel"
      leftToRight
      defaultHelperText={t('views.phoneHelp')}
      link={{
        href: phoneHref,
        label: (number) => t('views.callPhone', { number }),
        icon: <PhoneIcon size={smallIconSize} />,
      }}
    />
  );
};

export const MarkdownFieldEditor = ({
  attribute,
  value,
  disabled,
  required,
  error,
  helperText,
  onChange,
}: ValueEditorProps) => (
  <MarkdownEditor
    label={attributeLabel(attribute)}
    value={value}
    disabled={disabled}
    required={required}
    error={error}
    helperText={helperText}
    onChange={onChange}
  />
);

/**
 * Single select for a status attribute. Destinations the transition graph
 * forbids from the saved `baseline` are disabled; an empty selection inherits
 * `inheritedValue` (or clears the status when nothing is inherited). For a
 * saved record, `destinations` from the API also disables destinations whose
 * transition conditions are unmet and explains why.
 */
export const StatusEditor = ({
  attribute,
  config,
  value,
  baseline,
  inheritedValue,
  transitions = [],
  disabled,
  required,
  error,
  helperText,
  onChange,
}: ValueEditorProps & {
  config: StatusConfiguration;
  baseline: string | null;
  inheritedValue: string | null;
  /** Server-evaluated access to edges leaving the saved status. */
  transitions?: readonly StatusTransitionAccess[];
}) => {
  const { t } = useTranslation();
  const id = useId();
  const known =
    !value || config.options.some((option) => option.code === value);
  // Permission, role and separation-of-duties checks need the server; the
  // graph alone cannot say who may take an edge.
  const denialText = useStatusTransitionDenialText();
  const denial = (next: string) =>
    statusTransitionDenial(config, {
      attributeCode: attribute.code,
      baseline,
      inherited: inheritedValue,
      selected: next,
      transitions,
    });
  const allowed = (next: string) => !denial(next);
  const terminal =
    !config.options.some(
      (option) => option.code !== baseline && allowed(option.code),
    ) && !(inheritedValue !== baseline && allowed(''));
  return (
    <TextField
      id={id}
      fullWidth
      select
      label={attributeLabel(attribute)}
      value={value}
      disabled={disabled}
      required={required}
      slotProps={{ select: { readOnly: disabled } }}
      error={Boolean(error) || !known}
      helperText={
        error ??
        (!known ? (
          t('records.statusUnknown', { value })
        ) : (
          <FieldHint>
            {[
              helperText,
              terminal
                ? t('records.statusTerminal')
                : t('records.statusTransitions'),
            ]
              .filter(Boolean)
              .join(' ')}
          </FieldHint>
        ))
      }
      onChange={(event) => {
        if (!disabled && allowed(event.target.value))
          onChange(event.target.value);
      }}
    >
      <MenuItem value="" disabled={!allowed('')}>
        {inheritedValue
          ? t('records.statusInherit', {
              value: statusCodeLabel(config, inheritedValue) ?? inheritedValue,
            })
          : t('records.notSet')}
      </MenuItem>
      {!known && (
        <MenuItem value={value} disabled>
          {value}
        </MenuItem>
      )}
      {config.options.map((option) => {
        const denied = denial(option.code);
        return (
          <MenuItem
            key={option.code}
            value={option.code}
            disabled={Boolean(denied)}
          >
            {denied?.kind === 'access' ? (
              <ListItemText
                primary={statusOptionLabel(option)}
                secondary={denialText(denied)}
              />
            ) : (
              statusOptionLabel(option)
            )}
          </MenuItem>
        );
      })}
    </TextField>
  );
};
