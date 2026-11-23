import test from 'node:test';
import assert from 'node:assert/strict';
import { Client, ProtocolError, assertRequest, parseJson, parseResponse, validateDefinition } from '../dist/index.js';

test('strict parsing rejects duplicates, malformed text and excessive structure', () => {
  for (const text of ['{"a":1,"a":2}', '{"a":1,}', '[1,]', '1e999', 'true false', '"\\ud800"', '[ '.repeat(65) + '0' + ']'.repeat(65)]) {
    assert.throws(() => parseJson(text), ProtocolError, text);
  }
  assert.equal(parseJson('"麻雀 🀄"'), '麻雀 🀄');
  assert.equal(parseJson('{"__proto__":0}').__proto__, 0);
});

test('runtime request validation enforces bounds, literal tags and exact seed text', () => {
  assertRequest({ version: 1, op: 'environment_reset', seed: '18446744073709551615' });
  for (const value of [
    { version: 2, op: 'view' }, { version: 1, op: 'view', leaked: true },
    { version: 1, op: 'environment_reset', seed: '18446744073709551616' },
    { version: 1, op: 'environment_reset', seed: '01' },
    { version: 1, op: 'environment_reset', seed: 9007199254740992 },
    { version: 1, op: 'apply', session_id: 1, expected_revision: 0, action: { kind: 'discard', index: 14 } },
    { version: 1, op: 'step', session_id: 4294967296, expected_revision: 0 },
  ]) assert.throws(() => assertRequest(value), ProtocolError);
  assert.throws(() => validateDefinition('SignedValue', '9223372036854775808'), ProtocolError);
  validateDefinition('SignedValue', '-9223372036854775808');
});

test('untrusted responses and closed clients do not enter application state', async () => {
  assert.throws(() => parseResponse('{"status":"ok","version":1,"data":{"kind":"game","state":{}}}'), ProtocolError);
  const client = new Client({ request: () => '{"status":"wat"}' });
  await assert.rejects(client.command({ version: 1, op: 'view' }), ProtocolError);
  client.close();
  await assert.rejects(client.command({ version: 1, op: 'new' }), ProtocolError);
});

test('cyclic, accessor and sparse values are rejected before JSON serialization', () => {
  const cycle = {}; cycle.value = cycle;
  assert.throws(() => assertRequest(cycle), ProtocolError);
  const accessor = { version: 1, get op() { throw new Error('getter must not run'); } };
  assert.throws(() => assertRequest(accessor), ProtocolError);
  const sparse = []; sparse.length = 2; sparse[1] = 1;
  assert.throws(() => validateDefinition('WireAction', { kind: 'chi', tiles: sparse }), ProtocolError);
  sparse.other = 0;
  assert.throws(() => validateDefinition('WireAction', { kind: 'chi', tiles: sparse }), ProtocolError);
});

test('prototype names cannot bypass schema property and definition lookup', () => {
  for (const key of ['constructor', '__proto__', 'toString']) {
    assert.throws(() => assertRequest(parseJson(`{"op":"view","version":1,"${key}":42}`)), ProtocolError);
    assert.throws(() => validateDefinition(key, { arbitrary: true }), ProtocolError);
  }
});

test('integer wire values reject lossy decimal and exponent spellings like Rust', () => {
  assert.throws(() => assertRequest(parseJson('{"version":1,"op":"apply","session_id":1,"expected_revision":0,"action":{"kind":"discard","index":-0}}')), ProtocolError);
  for (const value of ['1.00000000000000001', '1e0', '1.0', '9007199254740993']) {
    assert.throws(() => parseResponse(`{"status":"error","version":${value},"code":"invalid_request","message":"test"}`), ProtocolError);
  }
});
