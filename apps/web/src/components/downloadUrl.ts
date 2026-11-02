/** Saves what a URL points to, under `filename` or the name it comes with. */
export const downloadUrl = (url: string, filename = '') => {
  const link = document.createElement('a');
  link.href = url;
  link.rel = 'noopener';
  link.download = filename;
  document.body.append(link);
  link.click();
  link.remove();
};
