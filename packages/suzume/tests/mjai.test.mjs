import test from 'node:test';
import assert from 'node:assert/strict';
import * as sdk from '../dist/index.js';

const encode = events => Buffer.from(events.map(event => JSON.stringify(event)).join('\n') + '\n');
const start = () => ({
  type: 'start_kyoku', bakaze: 'E', dora_marker: '1p', kyoku: 1, honba: 0,
  kyotaku: 0, oya: 0, scores: [25000, 25000, 25000, 25000],
  tehais: Array.from({ length: 4 }, () => Array(13).fill('?')),
});
const sample = () => [
  { type: 'start_game', names: ['検査 一', '検査 二', '検査 三', '検査 四'] }, start(),
  { type: 'tsumo', actor: 0, pai: '5mr', meta: { confidence: 0.75 } },
  { type: 'dahai', actor: 0, pai: '5mr', tsumogiri: true },
  { type: 'ryukyoku', deltas: [0, 0, 0, 0] }, { type: 'end_kyoku' }, { type: 'end_game' },
];
async function collect(stream) { const events = []; for await (const event of stream) events.push(event); return events; }
function* chunks(bytes, size = 7) { for (let offset = 0; offset < bytes.length; offset += size) yield bytes.subarray(offset, offset + size); }

test('mjai structure preserves red and unknown tiles and producer metadata', () => {
  assert.equal(typeof sdk.parseMjaiEvent, 'function');
  const event = sdk.parseMjaiEvent('{"type":"tsumo","actor":2,"pai":"5pr","meta":{"score":0.25},"__proto__":{"safe":true}}');
  assert.equal(event.type, 'tsumo'); assert.equal(event.pai, '5pr');
  assert.equal(event.meta.score, 0.25); assert.equal(event.__proto__.safe, true);
  assert.equal({}.safe, undefined);
  assert.equal(sdk.parseMjaiEvent('{"type":"tsumo","actor":0,"pai":"?"}').pai, '?');
  assert.equal(sdk.parseMjaiEvent('{"type":"start_game"}').type, 'start_game');
  const aliases = sdk.parseMjaiEvent('{"type":"hora","actor":1,"target":0,"uradora_markers":["5sr"]}');
  assert.deepEqual(aliases.uradora_markers, ['5sr']);
  const ending = sdk.parseMjaiEvent('{"type":"end_game","scores":[25000,25000,25000,25000],"_suzume":{"reason":"completed","kyotaku":0,"metrics":{"confidence":0.25}}}');
  assert.equal(ending._suzume.metrics.confidence, 0.25);
});

test('mjai rejects malformed fields, duplicate metadata keys and lossy integer spellings', () => {
  assert.equal(typeof sdk.parseMjaiEvent, 'function');
  const invalid = [
    '{"type":"tsumo","actor":4,"pai":"1m"}', '{"type":"tsumo","actor":0,"pai":"0m"}',
    '{"type":"tsumo","actor":0,"pai":"6mr"}', '{"type":"tsumo","actor":0}',
    '{"type":"dahai","actor":0,"pai":"1m","tsumogiri":1}',
    '{"type":"chi","actor":1,"target":0,"pai":"1m","consumed":["2m"]}',
    '{"type":"hora","actor":1,"target":0,"deltas":[1000,-1000,0]}',
    '{"type":"hora","actor":1,"target":0,"ura_markers":[],"uradora_markers":[]}',
    '{"type":"start_game","names":["a","b","c"]}', '{"type":"unknown"}',
    '{"type":"none","meta":{"a":1,"a":2}}',
    '{"type":"tsumo","actor":1.0,"pai":"1m"}',
    '{"type":"tsumo","actor":1e0,"pai":"1m"}',
    '{"type":"tsumo","actor":-0,"pai":"1m"}',
    '{"type":"none","meta":9007199254740993}',
    '{"type":"none","meta":1e999}', '{"type":"none","meta":"\\ud800"}',
    '{"type":"end_game","_suzume":null}',
    '{"type":"end_game","_suzume":{"reason":"other"}}',
    '{"type":"end_game","_suzume":{"kyotaku":1.0}}',
    '{"type":"end_game","scores":[25000,25000,25000,25000.0]}',
    '{"type":"end_game","scores":[25000,25000,25000]}',
  ];
  for (const text of invalid) assert.throws(() => sdk.parseMjaiEvent(text), sdk.ProtocolError, text);
  assert.throws(() => sdk.parseMjaiEvent(JSON.stringify({ type: 'none', meta: 'a'.repeat(1_048_576) })), /bytes|limit/i);
  // The existing API keeps its integer-only wire contract.
  assert.throws(() => sdk.parseJson('{"value":0.25}'), sdk.ProtocolError);
});

test('mjai streaming accepts split UTF-8, CRLF and an un-terminated final line', async () => {
  assert.equal(typeof sdk.readMjaiEvents, 'function');
  const expected = sample();
  const bytes = Buffer.from(expected.map(event => JSON.stringify(event)).join('\r\n'));
  const actual = await collect(sdk.readMjaiEvents(chunks(bytes, 1)));
  assert.equal(actual.length, 7);
  assert.deepEqual(JSON.parse(JSON.stringify(actual)), expected);
});

test('mjai streaming validates lifecycle and distinguishes declared interruption', async () => {
  assert.equal(typeof sdk.readMjaiEvents, 'function');
  for (const events of [
    [], [{ type: 'end_game' }], sample().slice(0, -1),
    [...sample(), { type: 'none' }],
    [{ type: 'start_game' }, start(), { type: 'end_game' }],
    [{ type: 'start_game' }, start(), { type: 'end_kyoku' }, { type: 'end_game' }],
    [{ type: 'start_game' }, { type: 'start_game' }],
    [{ type: 'start_game' }, { type: 'tsumo', actor: 0, pai: '1m' }],
  ]) await assert.rejects(collect(sdk.readMjaiEvents(chunks(encode(events)))), sdk.ProtocolError);
  for (const reason of ['interrupted', 'truncated']) {
    const events = [{ type: 'start_game' }, start(), { type: 'end_game', _suzume: { reason, complete: false } }];
    assert.equal((await collect(sdk.readMjaiEvents(chunks(encode(events))))).length, 3);
  }
  await assert.rejects(collect(sdk.readMjaiEvents([encode([
    { type: 'start_game' }, { type: 'end_game', _suzume: { reason: 'truncated', complete: true } },
  ])])), /complete|reason/);
  assert.equal((await collect(sdk.readMjaiEvents([encode([{ type: 'start_game' }, { type: 'end_game' }])]))).length, 2);
});

test('mjai limits and malformed stream diagnostics identify the failing line', async () => {
  assert.equal(typeof sdk.readMjaiEvents, 'function');
  const bytes = encode(sample());
  for (const limits of [{ maxLineBytes: 10 }, { maxRecords: 2 }, { maxTotalBytes: 10 }, { maxRecords: 0 }]) {
    await assert.rejects(collect(sdk.readMjaiEvents(chunks(bytes), limits)), sdk.ProtocolError);
  }
  await assert.rejects(collect(sdk.readMjaiEvents([Buffer.from([255])])), /UTF-8/);
  await assert.rejects(collect(sdk.readMjaiEvents([Buffer.from('{"type":"start_game"}\n{"type":"tsumo","actor":8,"pai":"1m"}\n')])), /line 2.*tsumo/i);
  await assert.rejects(collect(sdk.readMjaiEvents([Buffer.from('{"type":"start_game"}\n\n')])), /line 2/i);
});

test('mjai structural streaming does not claim to validate tile movement', async () => {
  assert.equal(typeof sdk.readMjaiEvents, 'function');
  const events = sample();
  events[2].actor = 2; // This contradicts the dealer turn and must be rejected by Rust, not this parser.
  assert.equal((await collect(sdk.readMjaiEvents([encode(events)]))).length, 7);
});
