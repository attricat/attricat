import i18n from '../../../i18n';

/** Only opaque six-digit sRGB hex is safe to use as a swatch fill. */
export const parseColor = (value: unknown): string | undefined =>
  typeof value === 'string' &&
  value.length === 7 &&
  /^#[0-9a-f]{6}$/i.test(value)
    ? value.toLowerCase()
    : undefined;

// Native color inputs require a valid value, even while the form is unset.
// This is only a picker seed: opening the picker never writes it to the form.
export const COLOR_PICKER_SEED = '#000000';

export const validateColor = (value: string) =>
  value === '' || parseColor(value) ? undefined : i18n.t('views.colorInvalid');
