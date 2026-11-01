/**
 * Asks the browser for one file of the given media types. Resolves undefined
 * when the person cancels. Call it from a click or a confirmation they just
 * answered: browsers only open the picker in response to a person's action.
 */
export const pickFile = (accepts: readonly string[]) =>
  new Promise<File | undefined>((resolve) => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = accepts.join(',');
    input.addEventListener('change', () => resolve(input.files?.[0]));
    input.addEventListener('cancel', () => resolve(undefined));
    input.click();
  });
