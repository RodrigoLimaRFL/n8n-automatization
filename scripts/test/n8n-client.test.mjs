import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createN8nClient, DEFAULT_FORM_ID } from '../n8n-client.mjs';

function fakeFetch(respond) {
  const calls = [];
  const fn = async (url, init) => {
    calls.push({ url, init });
    return respond(url, init);
  };
  return { fn, calls };
}

test('submitRun posts multipart fields for "Last N days"', async () => {
  const f = fakeFetch(() => ({ ok: true, status: 200 }));
  const n8n = createN8nClient({ fetch: f.fn });
  assert.equal(await n8n.submitRun(5), true);
  const [{ url, init }] = f.calls;
  assert.equal(url, `http://localhost:5678/form/${DEFAULT_FORM_ID}`);
  assert.equal(init.method, 'POST');
  assert.ok(init.body instanceof FormData);
  assert.equal(init.body.get('field-0'), 'Last N days');
  assert.equal(init.body.get('field-1'), '5');
});

test('submitRun for all emails omits field-1', async () => {
  const f = fakeFetch(() => ({ ok: true }));
  const n8n = createN8nClient({ fetch: f.fn, baseUrl: 'http://n8n.test:1234/', formId: 'abc' });
  assert.equal(await n8n.submitRun(0), true);
  assert.equal(f.calls[0].url, 'http://n8n.test:1234/form/abc');
  assert.equal(f.calls[0].init.body.get('field-0'), 'All emails');
  assert.equal(f.calls[0].init.body.has('field-1'), false);
});

test('submitRun is false on a non-2xx answer or a network error', async () => {
  const bad = createN8nClient({ fetch: fakeFetch(() => ({ ok: false, status: 404 })).fn });
  assert.equal(await bad.submitRun(1), false);
  const down = createN8nClient({ fetch: fakeFetch(() => { throw new Error('ECONNREFUSED'); }).fn });
  assert.equal(await down.submitRun(1), false);
});

test('isHealthy checks /healthz', async () => {
  const f = fakeFetch(() => ({ ok: true }));
  assert.equal(await createN8nClient({ fetch: f.fn }).isHealthy(), true);
  assert.equal(f.calls[0].url, 'http://localhost:5678/healthz');
  const down = createN8nClient({ fetch: fakeFetch(() => { throw new Error('down'); }).fn });
  assert.equal(await down.isHealthy(), false);
  const sick = createN8nClient({ fetch: fakeFetch(() => ({ ok: false })).fn });
  assert.equal(await sick.isHealthy(), false);
});
