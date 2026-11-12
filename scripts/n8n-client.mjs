// Minimal n8n client for the bridge: submit the "run now" form and check /healthz.
// `fetch` is injected so tests never touch the network.

export const DEFAULT_N8N_URL = 'http://localhost:5678';
export const DEFAULT_FORM_ID = '5f0c2b7e-3a8d-4e51-9c6a-2d7f1e4b8a90';

export function createN8nClient({
  fetch,
  baseUrl = DEFAULT_N8N_URL,
  formId = DEFAULT_FORM_ID,
  timeoutMs = 5000,
  healthTimeoutMs = 1500,
}) {
  const root = baseUrl.replace(/\/+$/, '');

  return {
    /** Submit the form for `days` (0 = all emails). Returns true when n8n answered 200. */
    async submitRun(days) {
      const form = new FormData();
      form.set('field-0', days === 0 ? 'All emails' : 'Last N days');
      if (days > 0) form.set('field-1', String(days));
      try {
        const res = await fetch(`${root}/form/${formId}`, {
          method: 'POST',
          body: form,
          signal: AbortSignal.timeout(timeoutMs),
        });
        return res.ok;
      } catch {
        return false;
      }
    },

    /** True when n8n answers /healthz. */
    async isHealthy() {
      try {
        const res = await fetch(`${root}/healthz`, { signal: AbortSignal.timeout(healthTimeoutMs) });
        return res.ok;
      } catch {
        return false;
      }
    },
  };
}
