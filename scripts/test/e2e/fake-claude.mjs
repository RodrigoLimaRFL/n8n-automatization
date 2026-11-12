// Fake `claude -p` for batch prompts. Reads <email id="..."> blocks; markers in the body decide the answer.
import fs from 'node:fs';
const args = process.argv.slice(2);
const prompt = fs.readFileSync(0, 'utf8');
const emails = [...prompt.matchAll(/<email id="([^"]+)">([\s\S]*?)<\/email>/g)].map((m) => ({ id: m[1], text: m[2] }));
fs.appendFileSync(process.env.FAKE_LOG || 'fake-claude.log', JSON.stringify({ n: emails.length, ids: emails.map((e) => e.id), effort: args[args.indexOf('--effort') + 1], bulkHints: emails.filter((e) => /divulgação: sim/.test(e.text)).map((e) => e.id) }) + '\n');

const fail = (msg) => { process.stdout.write(JSON.stringify({ type: 'result', subtype: 'error', is_error: true, result: msg })); process.exit(0); };
if (emails.some((e) => e.text.includes('FAKE:FAIL'))) fail('simulated failure');

const tasksMatch = prompt.match(/<tarefas_abertas>([\s\S]*?)<\/tarefas_abertas>/);
const tasks = tasksMatch ? JSON.parse(tasksMatch[1]) : [];
const find = (kind) => tasks.find((t) => (kind === 'calendar' ? t.date : !t.date));
const base = { details: 'Detalhes de teste', start_time: null, end_time: null };
const actions = [];
const has = (e, k) => e.text.includes('FAKE:' + k);
const same = emails.filter((e) => has(e, 'SAME'));
if (same.length) actions.push({ ...base, action: 'create', existing_task_id: null, title: 'Apresentação da Equipe 1', type: 'evento', date: '2026-12-04', start_time: '15:30', end_time: null, email_ids: same.map((e) => e.id) });
for (const e of emails) {
  const ids = [e.id];
  if (has(e, 'CREATE_EVENT')) actions.push({ ...base, action: 'create', existing_task_id: null, title: 'Prova de Cálculo', type: 'prova', date: '2026-11-20', start_time: '14:00', end_time: '16:00', email_ids: ids });
  if (has(e, 'CREATE_LIST')) actions.push({ ...base, action: 'create', existing_task_id: null, title: 'Ler capítulo 3', type: 'entrega', date: null, email_ids: ids });
  if (has(e, 'CREATE_ALLDAY')) actions.push({ ...base, action: 'create', existing_task_id: null, title: 'Pagar matrícula', type: 'burocracia', date: '2026-11-25', email_ids: ids });
  if (has(e, 'UPDATE_EVENT')) { const t = find('calendar'); actions.push({ ...base, action: 'update', existing_task_id: t.id, title: t.title, type: t.type, date: '2026-11-27', start_time: '10:00', end_time: '12:00', email_ids: ids }); }
  if (has(e, 'PROMOTE')) { const t = find('list'); actions.push({ ...base, action: 'update', existing_task_id: t.id, title: t.title, type: t.type, date: '2026-11-30', start_time: null, end_time: '23:59', email_ids: ids }); }
  if (has(e, 'CANCEL_EVENT')) { const t = find('calendar'); actions.push({ ...base, action: 'cancel', existing_task_id: t.id, title: t.title, type: t.type, date: t.date, email_ids: ids }); }
}
const out = { emails: emails.map((e) => ({ email_id: e.id, sender_is_person: !/De: [^\n]*(news@|promo@)/.test(e.text) })), actions };
process.stdout.write(JSON.stringify({ type: 'result', subtype: 'success', is_error: false, result: JSON.stringify(out), structured_output: out, duration_ms: 5, usage: { input_tokens: 10, output_tokens: 10 } }));
