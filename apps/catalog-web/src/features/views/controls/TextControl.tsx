import {
  IconButton,
  InputAdornment,
  Link,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import type { ComponentProps, ReactNode } from 'react';
import { attributeLabel } from '../../entities/entityDisplay';
import type { ValueEditorProps } from '../components/componentTypes';
import { EachValue } from './EachValue';

/** Opens a field's value, such as a website, mail client or phone call. */
export type TextControlLink = {
  href: (value: string) => string | undefined;
  label: (value: string) => string;
  icon: ReactNode;
  newTab?: boolean;
};

/**
 * Single-line string editor for controls that only change the input mode.
 * With `link`, a valid value can be opened from a button inside the field, so
 * an always-editable field keeps the action its display would offer.
 */
export const TextControlEditor = ({
  attribute,
  value,
  disabled,
  required,
  error,
  helperText,
  onChange,
  type,
  validate,
  defaultHelperText,
  leftToRight = false,
  link,
}: ValueEditorProps & {
  type: 'email' | 'tel' | 'url';
  /** Shows the form's validation message while typing, before Save. */
  validate?: (value: string) => string | undefined;
  defaultHelperText?: string;
  leftToRight?: boolean;
  link?: TextControlLink;
}) => {
  const message = error ?? validate?.(value);
  const target = link && !message ? link.href(value.trim()) : undefined;
  const linkLabel = target && link ? link.label(value.trim()) : undefined;
  return (
    <TextField
      fullWidth
      type={type}
      label={attributeLabel(attribute)}
      value={value}
      disabled={disabled}
      required={required}
      error={Boolean(message)}
      helperText={message ?? helperText ?? defaultHelperText}
      onChange={(event) => {
        if (!disabled) onChange(event.target.value);
      }}
      slotProps={{
        input: {
          endAdornment: target && link && (
            <InputAdornment position="end">
              <Tooltip title={linkLabel}>
                <IconButton
                  aria-label={linkLabel}
                  component="a"
                  edge="end"
                  href={target}
                  {...(link.newTab
                    ? { target: '_blank', rel: 'noopener noreferrer' }
                    : {})}
                >
                  {link.icon}
                </IconButton>
              </Tooltip>
            </InputAdornment>
          ),
        },
        htmlInput: {
          inputMode: type,
          autoCapitalize: 'none',
          spellCheck: false,
          dir: leftToRight ? 'ltr' : undefined,
        },
      }}
    />
  );
};

/**
 * Stored text that becomes a link when `href` accepts it. Invalid values stay
 * readable as text. Multi-valued projections render one value per line.
 */
export const LinkedTextValue = ({
  value,
  href,
  linkProps,
  leftToRight = false,
}: {
  value: unknown;
  href: (value: unknown) => string | undefined;
  linkProps?: (text: string) => ComponentProps<typeof Link>;
  leftToRight?: boolean;
}) => (
  <EachValue
    value={value}
    renderItem={(item) => {
      const text =
        typeof item === 'object' ? JSON.stringify(item) : String(item);
      const target = href(item);
      // Keep left-to-right values such as phone numbers intact in RTL layouts.
      const dir = leftToRight ? 'ltr' : undefined;
      const sx = {
        overflowWrap: 'anywhere',
        unicodeBidi: leftToRight ? 'isolate' : undefined,
      } as const;
      return target ? (
        <Link
          href={target}
          dir={dir}
          variant="body2"
          sx={sx}
          // Table rows and cards are clickable; following a link must not activate them.
          onClick={(event) => event.stopPropagation()}
          onKeyDown={(event) => event.stopPropagation()}
          {...linkProps?.(text)}
        >
          {text}
        </Link>
      ) : (
        <Typography dir={dir} variant="body2" sx={sx}>
          {text}
        </Typography>
      );
    }}
  />
);
