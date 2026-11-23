import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { parseLogRecord, readLogRecords, readTransitions, ProtocolError } from '../dist/index.js';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const sample = fileURLToPath(new URL('../../../examples/replay-eight-actions.jsonl', import.meta.url));
const bytes = readFileSync(sample);
async function collect(stream) { const rows = []; for await (const row of stream) rows.push(row); return rows; }
function* chunks(data, size = 7) { for (let i = 0; i < data.length; i += size) yield data.subarray(i, i + size); }

test('streaming parser reads native JSONL over arbitrary UTF-8 chunk boundaries', async () => {
  const records = await collect(readLogRecords(chunks(bytes)));
  assert.equal(records.length, 10); assert.equal(records[0].record, 'replay_header');
  assert.equal(records[9].reason, 'truncated');
  assert.equal(records[1].decision.config.algorithm, 'random');
});

test('streaming parser refuses missing footer, duplicate keys and configured limits', async () => {
  const lines = bytes.toString('utf8').trimEnd().split('\n');
  const partial = Buffer.from(lines.slice(0, -1).join('\n'));
  await assert.rejects(collect(readLogRecords(chunks(partial))), /Missing footer/);
  await assert.rejects(collect(readLogRecords(chunks(bytes), { maxLineBytes: 100 })), ProtocolError);
  await assert.rejects(collect(readLogRecords(chunks(bytes), { maxRecords: 2 })), ProtocolError);
  await assert.rejects(collect(readLogRecords(chunks(bytes), { maxTotalBytes: 100 })), ProtocolError);
  assert.throws(() => parseLogRecord(lines[0].replace('"version":1', '"version":1,"version":1')), /Duplicate/);
  const reversed = [...lines]; [reversed[1], reversed[2]] = [reversed[2], reversed[1]];
  await assert.rejects(collect(readLogRecords(chunks(Buffer.from(reversed.join('\n'))))), /sequence/);
  await assert.rejects(collect(readLogRecords([Buffer.from([255])])), /UTF-8/);
});

test('native replay exports transitions consumed by the typed streaming parser', async () => {
  const directory = mkdtempSync(join(tmpdir(), 'suzume-sdk-log-'));
  try {
    const path = join(directory, 'dataset.jsonl');
    const native = spawnSync('cargo', ['run', '--quiet', '--locked', '-p', 'suzume-api', '--bin', 'suzume-api', '--', 'dataset', sample, '--output', path], { cwd: root, encoding: 'utf8', timeout: 120_000 });
    assert.equal(native.status, 0, native.stderr);
    const rows = await collect(readTransitions(chunks(readFileSync(path), 13)));
    assert.equal(rows.length, 8); assert.equal(rows.at(-1).truncated, true);
    assert.ok(rows.at(-1).final_observation);
    for (const row of rows) assert.equal(row.observation.legal_mask[row.action_id], true);
    await assert.rejects(collect(readTransitions([Buffer.from(rows.slice(0, -1).map(row => JSON.stringify(row)).join('\n'))])), /Dataset ended/);
  } finally { rmSync(directory, { recursive: true, force: true }); }
});
