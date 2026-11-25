import i18n from '.';

/**
 * Lists items in the active language, such as "Sheet, Profile and Part", or
 * with `disjunction` "Sheet, Profile or Part".
 */
export const formatList = (
  items: string[],
  type: Intl.ListFormatType = 'conjunction',
) => new Intl.ListFormat(i18n.language, { type }).format(items);
