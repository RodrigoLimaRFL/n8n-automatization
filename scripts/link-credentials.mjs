// Links the credentials in workflows/*.json to the ones saved in THIS n8n, by name.
//
// Workflow files refer to a credential by { id, name }, and n8n only looks at the id,
// which is different on every machine. This script reads your credentials' ids
// (never their secrets) and writes them into the workflow files, then imports them.
//
//   npm run link
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const n8n = (...args) => execFileSync(process.execPath, ['--env-file=.env', 'node_modules/n8n/bin/n8n', ...args], { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });

// 1. Read id/name/type of every saved credential. The export also contains the encrypted
//    secrets, so it goes to a temp file that is deleted right away.
const tmp = path.join(os.tmpdir(), `n8n-credentials-${process.pid}.json`);
let saved;
try {
  n8n('export:credentials', '--all', `--output=${tmp}`);
  saved = JSON.parse(fs.readFileSync(tmp, 'utf8'));
} finally {
  fs.rmSync(tmp, { force: true });
}
const byTypeAndName = new Map(saved.map((c) => [`${c.type}|${c.name}`, c.id]));

// 2. Fill in the ids in each workflow file.
const dir = path.join(root, 'workflows');
const missing = new Set();
const changedFiles = [];
for (const file of fs.readdirSync(dir).filter((f) => f.endsWith('.json'))) {
  const full = path.join(dir, file);
  const wf = JSON.parse(fs.readFileSync(full, 'utf8'));
  let changed = false;
  for (const node of wf.nodes || []) {
    for (const [type, ref] of Object.entries(node.credentials || {})) {
      const id = byTypeAndName.get(`${type}|${ref.name}`);
      if (!id) { missing.add(`"${ref.name}" (${type})`); continue; }
      if (ref.id !== id) { ref.id = id; changed = true; }
    }
  }
  if (changed) {
    fs.writeFileSync(full, JSON.stringify(wf, null, 2) + '\n');
    changedFiles.push(file);
  }
}

// 3. Import the updated files.
for (const file of changedFiles) {
  n8n('import:workflow', `--input=workflows/${file}`);
  console.log(`Linked and imported ${file}`);
}
if (!changedFiles.length) console.log('All credential links were already up to date.');
if (missing.size) {
  console.log('\nNot found in this n8n (create them with exactly these names, then run again):');
  for (const m of missing) console.log('  - ' + m);
}
console.log('\nReload the editor, then Publish 02 and 01 again so the running version uses the changes.');
