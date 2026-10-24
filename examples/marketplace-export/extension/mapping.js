// The `export_mapping` panel: one frame per export destination on the
// extension's management page. Panels cannot call server commands, so the
// mapping is saved straight to extension storage under `field-map:<export_id>`,
// where the exporter reads it. Only Amazon uses `product_type` today.

const key = (exportId) => `field-map:${exportId}`;

export const mount = (root, attricat) => {
  const exportId = attricat.context.export_id;
  let revision = null;

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

  const load = async () => {
    const entry = await attricat.storage.get({ key: key(exportId) });
    revision = entry?.revision ?? null;
    input.value = entry?.value?.product_type ?? '';
  };

  form.addEventListener('submit', async (event) => {
    event.preventDefault();
    status.textContent = 'Saving…';
    try {
      const saved = await attricat.storage.set({
        key: key(exportId),
        value: { version: 1, product_type: input.value.trim() },
        expected_revision: revision,
      });
      revision = saved.revision;
      status.textContent = 'Saved.';
    } catch (error) {
      // A revision conflict means someone else saved first: show their value.
      status.textContent = `Not saved: ${error.message}`;
      await load();
    }
  });

  load().catch((error) => {
    status.textContent = `Could not load the mapping: ${error.message}`;
  });
  return () => root.replaceChildren();
};
