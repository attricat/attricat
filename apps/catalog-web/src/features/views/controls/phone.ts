// This is a conservative dial-target parser, not phone-number verification.
// National numbers need an explicit region and intentionally remain plain text.
const MAX_DIAL_TEXT_LENGTH = 128;
const internationalPhonePattern =
  /^\+([1-9][0-9 () .-]*)(?: *(?:ext\.?|x) *([0-9]{1,10}))?$/i;
const internationalDigitsPattern = /^[1-9][0-9]{1,14}$/;

export const phoneHref = (value: unknown): string | undefined => {
  if (typeof value !== 'string' || value.length > MAX_DIAL_TEXT_LENGTH)
    return undefined;
  // Reject control characters before trimming; never allow URI parameters.
  if (
    [...value].some(
      (character) =>
        character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127,
    )
  )
    return undefined;
  const match = internationalPhonePattern.exec(value.trim());
  if (!match) return undefined;
  const digits = match[1].replace(/[ () .-]/g, '');
  if (!internationalDigitsPattern.test(digits)) return undefined;
  return `tel:+${digits}${match[2] ? `;ext=${match[2]}` : ''}`;
};
