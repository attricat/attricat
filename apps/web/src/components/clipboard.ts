/**
 * Copies through a hidden text area for pages without the Clipboard API,
 * which browsers only expose in secure contexts (for example the dev app
 * opened over plain HTTP on a LAN address).
 */
const copyWithTextArea = (value: string) => {
  const textArea = document.createElement('textarea');
  textArea.value = value;
  textArea.setAttribute('readonly', '');
  textArea.style.position = 'fixed';
  textArea.style.opacity = '0';
  textArea.style.pointerEvents = 'none';
  const previousFocus =
    document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
  // Inside a modal dialog its focus trap would pull focus back from the body.
  const container = previousFocus?.closest('[role="dialog"]') ?? document.body;
  container.append(textArea);
  try {
    textArea.focus();
    textArea.select();
    // execCommand is deprecated but is the only copy path outside secure contexts.
    if (!document.execCommand('copy')) throw new Error('Copy was rejected');
  } finally {
    textArea.remove();
    previousFocus?.focus();
  }
};

export const copyToClipboard = async (value: string) => {
  if (navigator.clipboard) {
    await navigator.clipboard.writeText(value);
    return;
  }
  copyWithTextArea(value);
};
