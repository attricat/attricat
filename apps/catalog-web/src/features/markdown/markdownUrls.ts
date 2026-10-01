/** Reject browser-normalized custom protocols and network-path references. */
export const markdownUrlTransform = (url: string): string => {
  const normalized = Array.from(url)
    .filter(
      (character) =>
        character.charCodeAt(0) > 32 && character.charCodeAt(0) !== 127,
    )
    .join('');
  if (normalized.startsWith('//') || normalized.includes('\\')) return '';
  return /^(?:https?:|[^:]*$)/i.test(normalized) ? url : '';
};
