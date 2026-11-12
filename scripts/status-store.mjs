// File persistence for the run status snapshot (logs/run-status.json).
import fs from 'node:fs';
import path from 'node:path';

/** Read the snapshot, or null when missing/corrupt. */
export function loadSnapshot(file) {
  try {
    return JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch {
    return null;
  }
}

/** Returns a persist(snapshot) callback that writes `file` atomically; failures are reported, not thrown. */
export function createSnapshotWriter(file, onError = console.error) {
  return (snapshot) => {
    try {
      fs.mkdirSync(path.dirname(file), { recursive: true });
      const tmp = `${file}.tmp`;
      fs.writeFileSync(tmp, JSON.stringify(snapshot, null, 2));
      fs.renameSync(tmp, file);
    } catch (err) {
      onError(`Could not save ${file}: ${err.message}`);
    }
  };
}
