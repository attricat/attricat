// The `export_mapping` panel: one frame per export destination on the
// extension's management page. It reads and saves the destination's mapping
// through the `field-map` server command, which validates it and keeps it in
// extension storage, where the exporter reads it. Only Amazon uses
// `product_type` today.

export const mount = (root, attricat) => {
  const exportId = attricat.context.export_id;
  const fieldMap = (payload) =>
    attricat.command({ command_id: 'field-map', payload: { export_id: exportId, ...payload } });

  root.style.font = '14px system-ui, sans-serif';
  root.style.color = 'CanvasText';
  root.innerHTML = `
    <form>
      <label>Product type for ${exportId}
        <input name="product_type" maxlength="64" placeholder="PRODUCT" />
      </label>
      <button type="submit">Save</button>
      <p role="status" aria-live="polite"></p>
    </form>`;
  const form = root.querySelector('form');
  const input = form.elements.product_type;
  const status = root.querySelector('[role=status]');

  form.addEventListener('submit', async (event) => {
    event.preventDefault();
    status.textContent = 'Saving…';
    try {
      const result = await fieldMap({ product_type: input.value.trim() });
      status.textContent = result.error ?? 'Saved.';
    } catch (error) {
      status.textContent = `Not saved: ${error.message}`;
    }
  });

  fieldMap({})
    .then((result) => {
      input.value = result.product_type ?? '';
    })
    .catch((error) => {
      status.textContent = `Could not load the mapping: ${error.message}`;
    });
  return () => root.replaceChildren();
};
