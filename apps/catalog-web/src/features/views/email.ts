// A single ASCII dot-atom mailbox. No display names, recipient lists or URI headers.
const emailPattern =
  /^[A-Za-z0-9!#$%&'*+/=?^_`{|}~-]+(?:\.[A-Za-z0-9!#$%&'*+/=?^_`{|}~-]+)*@(?:[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?\.)*[A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?$/;
const maximumEmailLength = 254;
const maximumLocalLength = 64;
const maximumDomainLabelLength = 63;

export const isEmailAddress = (value: string) => {
  const [local, domain] = value.split('@');
  return (
    value === value.trim() &&
    value.length <= maximumEmailLength &&
    local.length <= maximumLocalLength &&
    emailPattern.test(value) &&
    domain.split('.').every((label) => label.length <= maximumDomainLabelLength)
  );
};

export const emailHref = (value: unknown): string | undefined =>
  typeof value === 'string' && isEmailAddress(value)
    ? `mailto:${encodeURIComponent(value).replace(/%40/g, '@')}`
    : undefined;
