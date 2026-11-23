import test from 'node:test';
import assert from 'node:assert/strict';
import * as sdk from '../dist/index.js';

const source = '{ "type":"start_game", "extra":{"huge":9007199254740993e0,"tiny":1e-400,"exponent":1e20,"exact":0.12345678901234567890123456789,"absent":null},"note":"改行\\n保持" }';
const ending = '{"type":"end_game"}';
const bytes = text => Buffer.from(text, 'utf8');
const document = lines => lines.join('\n') + '\n';
const envelope = (event, index = 1, annotations = '{}') => `{"record":"annotated_mjai","schema_version":1,"index":${index},"event":${event},"annotations":${annotations}}`;
async function collect(stream) { const records = []; for await (const record of stream) records.push(record); return records; }
function* chunks(input, size = 7) { for (let i = 0; i < input.length; i += size) yield input.subarray(i, i + size); }

test('source records retain the exact numeric spelling, aliases, nulls and unknown fields', () => {
  assert.equal(typeof sdk.parseMjaiRecord, 'function');
  const record = sdk.parseMjaiRecord(source);
  assert.equal(record.json, source);
  assert.equal(record.event.extra.tiny, 0); // Analysis is approximate; the stored text remains authoritative.
  assert.notEqual(JSON.stringify(record.event), source);
  const alias = '{"type":"hora","actor":0,"target":1,"deltas":null,"uradora_markers":["5mr"],"extra":{"n":1e20}}';
  assert.equal(sdk.parseMjaiRecord(alias).json, alias);
  assert.throws(() => sdk.parseMjaiRecord('{"type":"none","meta":9007199254740993}'), sdk.ProtocolError);
});

test('annotated parsing and encoding preserve the event and annotation source independently', () => {
  assert.equal(typeof sdk.encodeAnnotatedMjaiRecord, 'function');
  const annotations = '{ "analyzer":{"confidence":1e-400,"value":9007199254740993e0,"reason":"注釈","nothing":null} }';
  const encoded = sdk.encodeAnnotatedMjaiRecord(source, 1, annotations);
  assert.equal(encoded, envelope(source, 1, annotations));
  assert.equal(encoded.includes('\n'), false);
  const record = sdk.parseAnnotatedMjaiRecord(encoded);
  assert.equal(record.record, 'annotated_mjai'); assert.equal(record.schema_version, 1); assert.equal(record.index, 1);
  assert.equal(record.eventJson, source); assert.equal(record.annotationsJson, annotations);
  assert.equal(record.annotations.analyzer.confidence, 0);
  record.event.extra.tiny = 123; record.annotations.analyzer.confidence = 456;
  assert.equal(sdk.encodeAnnotatedMjaiRecord(record.eventJson, record.index, record.annotationsJson), encoded);
  const unusual = '{"event":{"fake":"event"},"a/b":{"~index":1.5},"__proto__":{"safe":true}}';
  const parsed = sdk.parseAnnotatedMjaiRecord(envelope(source, 1, unusual));
  assert.equal(parsed.annotationsJson, unusual); assert.equal({}.safe, undefined);
});

test('annotated records reject schema, sequence, duplicate and integer-token errors without normalization', () => {
  assert.equal(typeof sdk.parseAnnotatedMjaiRecord, 'function');
  const good = envelope('{"type":"none"}');
  const invalid = [
    good.replace('"schema_version":1', '"schema_version":2'),
    good.replace('"schema_version":1', '"schema_version":1.0'),
    good.replace('"schema_version":1', '"schema_version":1e0'),
    good.replace('"index":1', '"index":0'), good.replace('"index":1', '"index":-0'),
    good.replace('"index":1', '"index":1.0'), good.replace('"index":1', '"index":1e0'),
    good.replace('"index":1', '"index":4294967296'),
    good.replace('"record":"annotated_mjai"', '"record":"other"'),
    good.replace('"index":1,', ''), good.replace('"annotations":{}', '"annotations":null'),
    good.replace('"annotations":{}', '"annotations":[]'), good.replace('"annotations":{}', '"annotations":{"x":1,"x":2}'),
    good.replace('"index":1', '"index":1,"extra":true'), good.replace('"index":1', '"index":1,"index":1'),
    envelope('{"type":"tsumo","actor":1.0,"pai":"1m"}'),
    envelope('{"type":"none","meta":{"x":1,"x":2}}'),
    envelope('{"type":"none"}', 1, '{"bad":9007199254740993}'),
    envelope('{"type":"none"}', 1, '{"bad":1e999}'),
    envelope('"not an event"'),
  ];
  for (const input of invalid) assert.throws(() => sdk.parseAnnotatedMjaiRecord(input), sdk.ProtocolError, input);
  assert.equal(sdk.parseAnnotatedMjaiRecord(good.replace('"index":1', '"index":4294967295')).index, 4294967295);
});

test('single-line encoding rejects embedded physical lines and non-JSON surrounding whitespace', () => {
  assert.equal(typeof sdk.encodeAnnotatedMjaiRecord, 'function');
  for (const event of ['{"type":\n"none"}', '{"type":\r"none"}', '\u00a0{"type":"none"}', '{"type":"none"}\u00a0']) {
    assert.throws(() => sdk.encodeAnnotatedMjaiRecord(event, 1, '{}'), sdk.ProtocolError);
  }
  for (const annotations of ['{"note":\n"line"}', '\u00a0{}']) {
    assert.throws(() => sdk.encodeAnnotatedMjaiRecord(source, 1, annotations), sdk.ProtocolError);
  }
  assert.equal(sdk.encodeAnnotatedMjaiRecord(`\r\n ${source}\t`, 1, ' \t{}\r\n'), envelope(source));
  for (const index of [0, -1, -0, NaN, 1.5, 4294967296]) assert.throws(() => sdk.encodeAnnotatedMjaiRecord(source, index, '{}'), sdk.ProtocolError);
});

test('stream conversion preserves every source record and reports completion after asynchronous writes', async () => {
  assert.equal(typeof sdk.wrapMjaiRecords, 'function');
  const original = document([source, ending]);
  const wrapped = [];
  const report = await sdk.wrapMjaiRecords(chunks(bytes(original), 1), async line => { await Promise.resolve(); wrapped.push(line); });
  assert.deepEqual(report, { records: 2, droppedAnnotationRecords: 0 });
  assert.equal(wrapped.join(''), document([envelope(source), envelope(ending, 2)]));
  const parsed = await collect(sdk.readAnnotatedMjaiRecords(chunks(bytes(wrapped.join('')), 1)));
  assert.deepEqual(parsed.map(record => record.eventJson), [source, ending]);
  const restored = [];
  assert.deepEqual(await sdk.unwrapAnnotatedMjaiRecords(chunks(bytes(wrapped.join(''))), line => { restored.push(line); }), report);
  assert.equal(restored.join(''), original);
  const standard = await collect(sdk.readMjaiRecords(chunks(bytes(` \t${source}\r\n${ending}`), 1)));
  assert.deepEqual(standard.map(record => record.json), [source, ending]);
});

test('annotations cannot be silently discarded and explicit removal reports affected records', async () => {
  assert.equal(typeof sdk.unwrapAnnotatedMjaiRecords, 'function');
  const input = bytes(document([envelope(source, 1, '{"why":"理由"}'), envelope(ending, 2, '{"nil":null}')]));
  const rejected = [];
  await assert.rejects(sdk.unwrapAnnotatedMjaiRecords([input], line => { rejected.push(line); }), /annotation/i);
  assert.equal(rejected.length, 0);
  const restored = [];
  assert.deepEqual(await sdk.unwrapAnnotatedMjaiRecords([input], line => { restored.push(line); }, { dropAnnotations: true }),
    { records: 2, droppedAnnotationRecords: 2 });
  assert.equal(restored.join(''), document([source, ending]));
  await assert.rejects(sdk.unwrapAnnotatedMjaiRecords([input], () => {}, { dropAnnotations: 'true' }), sdk.ProtocolError);
});

test('annotated streams enforce consecutive indexes and complete event lifecycles', async () => {
  assert.equal(typeof sdk.readAnnotatedMjaiRecords, 'function');
  const rows = [envelope(source), envelope(ending, 2)];
  const invalid = [[], rows.slice(0, 1), [envelope(source, 2), rows[1]], [rows[0], envelope(ending, 1)],
    [rows[0], envelope(ending, 3)], [...rows, envelope('{"type":"none"}', 3)],
    [envelope(ending)], [envelope(source), envelope('{"type":"tsumo","actor":0,"pai":"1m"}', 2)]];
  for (const values of invalid) await assert.rejects(collect(sdk.readAnnotatedMjaiRecords([bytes(document(values))])), sdk.ProtocolError);
  await assert.rejects(collect(sdk.readAnnotatedMjaiRecords([Buffer.from([255])])), /UTF-8/);
  const broken = document([rows[0], envelope('{"type":"tsumo","actor":1.0,"pai":"1m"}', 2)]);
  await assert.rejects(collect(sdk.readAnnotatedMjaiRecords([bytes(broken)])), /line 2/);
});

test('input and output budgets include UTF-8 framing and wrapper overhead', async () => {
  assert.equal(typeof sdk.wrapMjaiRecords, 'function');
  const minimal = document(['{"type":"start_game"}', ending]);
  const wrapped = document([envelope('{"type":"start_game"}'), envelope(ending, 2)]);
  for (const limits of [{ maxLineBytes: 0 }, { maxRecords: 0 }, { maxTotalBytes: 0 }, { maxRecords: 1.5 }]) {
    await assert.rejects(sdk.wrapMjaiRecords([bytes(minimal)], () => {}, limits), sdk.ProtocolError);
  }
  await assert.rejects(sdk.wrapMjaiRecords([bytes(minimal)], () => {}, { maxLineBytes: 30 }), /line|bytes|limit/i);
  await assert.rejects(sdk.wrapMjaiRecords([bytes(minimal)], () => {}, { maxTotalBytes: bytes(minimal).length }), /total|limit/i);
  await assert.rejects(sdk.wrapMjaiRecords([bytes(minimal)], () => {}, { maxRecords: 1 }), /record|limit/i);
  const output = [];
  await sdk.wrapMjaiRecords([bytes(minimal)], line => { output.push(line); }, { maxTotalBytes: bytes(wrapped).length });
  assert.equal(output.join(''), wrapped);
  await assert.rejects(sdk.wrapMjaiRecords([bytes(minimal)], () => {}, { maxTotalBytes: bytes(wrapped).length - 1 }), /total|limit/i);
  const unwrappedInput = bytes(wrapped.trimEnd());
  const successful = [];
  await sdk.unwrapAnnotatedMjaiRecords([unwrappedInput], line => { successful.push(line); }, {}, { maxTotalBytes: unwrappedInput.length });
  assert.equal(successful.join(''), minimal);
  assert.throws(() => sdk.encodeAnnotatedMjaiRecord(source, 1, '{}', { maxLineBytes: 10 }), /line|bytes|limit/i);
  const huge = '{"type":"start_game","extra":"' + 'a'.repeat(1_048_500) + '"}';
  assert.throws(() => sdk.encodeAnnotatedMjaiRecord(huge, 1, '{}'), /line|bytes|limit/i);
});

test('incomplete input and failed sinks never produce a successful conversion report', async () => {
  assert.equal(typeof sdk.wrapMjaiRecords, 'function');
  const writes = [];
  await assert.rejects(sdk.wrapMjaiRecords([bytes(document([source]))], line => { writes.push(line); }), /end_game|incomplete/i);
  assert.equal(writes.length, 1); // The caller must keep this provisional output unpublished.
  const failed = new Error('synthetic sink failure');
  await assert.rejects(sdk.wrapMjaiRecords([bytes(document([source, ending]))], async () => { throw failed; }), /synthetic sink failure/);
  await assert.rejects(sdk.unwrapAnnotatedMjaiRecords([bytes(document([envelope(source), envelope(ending, 2)]))], () => { throw failed; }), /synthetic sink failure/);
});

test('existing event-only reading keeps internal CR compatibility while saved records reject it', async () => {
  const original = '{"type":\r"start_game"}\n{"type":"end_game"}\n';
  const events = await collect(sdk.readMjaiEvents([bytes(original)]));
  assert.deepEqual(events.map(event => event.type), ['start_game', 'end_game']);
  await assert.rejects(collect(sdk.readMjaiRecords([bytes(original)])), /physical line/);
  await assert.rejects(sdk.wrapMjaiRecords([bytes(original)], () => {}), /physical line/);
});

test('discard policy does not inherit authorization from the options prototype', async () => {
  const input = bytes(document([envelope(source, 1, '{"why":"保持"}'), envelope(ending, 2)]));
  const output = [];
  await assert.rejects(sdk.unwrapAnnotatedMjaiRecords([input], line => { output.push(line); }, Object.create({ dropAnnotations: true })), /annotation/i);
  assert.equal(output.length, 0);
});

test('discard policy remains the value at invocation despite input-side mutation', async () => {
  const input = bytes(document([envelope(source, 1, '{"why":"保持"}'), envelope(ending, 2)]));
  const options = { dropAnnotations: false };
  function* changingInput() { options.dropAnnotations = true; yield input; }
  await assert.rejects(sdk.unwrapAnnotatedMjaiRecords(changingInput(), () => {}, options), /annotation/i);
  const granted = { dropAnnotations: true };
  function* revokedInput() { granted.dropAnnotations = false; yield input; }
  const report = await sdk.unwrapAnnotatedMjaiRecords(revokedInput(), () => {}, granted);
  assert.deepEqual(report, { records: 2, droppedAnnotationRecords: 1 });
});

test('discard policy reads an own getter exactly once before consuming input', async () => {
  const input = bytes(document([envelope(source, 1, '{"why":"保持"}'), envelope(ending, 2)]));
  let deniedReads = 0;
  const denied = Object.defineProperty({}, 'dropAnnotations', { enumerable: true, get() { return ++deniedReads > 1; } });
  await assert.rejects(sdk.unwrapAnnotatedMjaiRecords([input], () => {}, denied), /annotation/i);
  assert.equal(deniedReads, 1);
  let grantedReads = 0;
  const granted = Object.defineProperty({}, 'dropAnnotations', { enumerable: true, get() { return ++grantedReads === 1; } });
  assert.deepEqual(await sdk.unwrapAnnotatedMjaiRecords([input], () => {}, granted), { records: 2, droppedAnnotationRecords: 1 });
  assert.equal(grantedReads, 1);
});
