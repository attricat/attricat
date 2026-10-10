export const copyToClipboard = async (value: string) => {
  if (!navigator.clipboard) throw new Error('Clipboard API is unavailable');
  await navigator.clipboard.writeText(value);
};
