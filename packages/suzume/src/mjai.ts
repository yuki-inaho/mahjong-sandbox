import type { MjaiEvent } from './generated/types.js';
import { protocolSchema } from './generated/schema.js';
import { readJsonLines, resolveLogLimits, type LogLimits } from './logs.js';
import { parseExtensionJson, ProtocolError, validateDefinition } from './validation.js';

/** Known fields have generated types; uninterpreted producer extensions stay unknown. */
export type MjaiEventRecord = MjaiEvent & Readonly<Record<string, unknown>>;

/** Parsed values are for inspection; json is the authoritative text for saving. */
export interface ParsedMjaiRecord {
  readonly event: MjaiEventRecord;
  readonly json: string;
}

/** Full record storage, not a player's private-information-filtered observation. */
export interface AnnotatedMjaiRecord {
  readonly record: 'annotated_mjai';
  readonly schema_version: 1;
  readonly index: number;
  readonly event: MjaiEventRecord;
  readonly annotations: Readonly<Record<string, unknown>>;
  readonly eventJson: string;
  readonly annotationsJson: string;
}

export interface MjaiTranscodeReport {
  readonly records: number;
  readonly droppedAnnotationRecords: number;
}

export interface UnwrapMjaiOptions {
  readonly dropAnnotations?: boolean;
}

type ByteChunks = AsyncIterable<Uint8Array> | Iterable<Uint8Array>;
type LineWriter = (line: string) => void | Promise<void>;
const encoder = new TextEncoder();
const annotationFields = new Set(['record', 'schema_version', 'index', 'event', 'annotations']);
const preservedPaths = new Set(['/event', '/annotations']);

type EventShape = { readonly properties: Readonly<Record<string, unknown>> & { readonly type: { readonly enum: readonly string[] } } };
const schema = protocolSchema as { readonly definitions: { readonly MjaiEvent: { readonly oneOf: readonly EventShape[] } } };
const fieldsByType = new Map(schema.definitions.MjaiEvent.oneOf.map(shape => [
  shape.properties.type.enum[0], new Set(Object.keys(shape.properties)),
]));
const has = (value: object, key: string): boolean => Object.prototype.hasOwnProperty.call(value, key);
const isObject = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === 'object' && !Array.isArray(value);

function requireIntegerTokens(paths: ReadonlySet<string>, fields: ReadonlySet<string>): void {
  for (const path of paths) {
    const field = path.split('/')[1];
    if (field !== undefined && fields.has(field)) throw new ProtocolError(`Integer event fields do not accept decimal, exponent or negative-zero tokens: ${path}`);
  }
}

function endReason(event: Readonly<Record<string, unknown>>, decimalPaths: ReadonlySet<string> = new Set()): 'completed' | 'truncated' | 'interrupted' {
  const meta = event['_suzume'];
  if (meta !== undefined && !isObject(meta)) throw new ProtocolError('_suzume must be an object');
  const reason = isObject(meta) && has(meta, 'reason') ? meta['reason'] : 'completed';
  if (reason !== 'completed' && reason !== 'truncated' && reason !== 'interrupted') throw new ProtocolError('Unsupported _suzume.reason');
  if (isObject(meta)) {
    if (has(meta, 'complete') && meta['complete'] !== (reason === 'completed')) throw new ProtocolError('_suzume.complete contradicts reason');
    if (has(meta, 'kyotaku')) {
      const sticks = meta['kyotaku'];
      if (typeof sticks !== 'number' || !Number.isInteger(sticks) || sticks < 0 || sticks > 4_294_967_295 || decimalPaths.has('/_suzume/kyotaku')) {
        throw new ProtocolError('_suzume.kyotaku must be an integer u32');
      }
    }
  }
  if (has(event, 'scores')) {
    const scores = event['scores'];
    if (!Array.isArray(scores) || scores.length !== 4 || scores.some(score => typeof score !== 'number' || !Number.isInteger(score) || score < -2_147_483_648 || score > 2_147_483_647)) {
      throw new ProtocolError('end_game scores must contain four i32 integers');
    }
    requireIntegerTokens(decimalPaths, new Set(['scores']));
  }
  return reason;
}

function parseRecord(text: string, line: number): MjaiEventRecord {
  let kind = 'json';
  try {
    const { value, decimalPaths } = parseExtensionJson(text);
    if (!isObject(value) || typeof value['type'] !== 'string') throw new ProtocolError('Expected an event object with a type');
    kind = value['type'];
    const fields = fieldsByType.get(kind);
    if (!fields) throw new ProtocolError('Unknown mjai event type');
    const projection: Record<string, unknown> = Object.create(null) as Record<string, unknown>;
    for (const field of fields) if (has(value, field)) projection[field] = value[field];
    requireIntegerTokens(decimalPaths, fields);
    if (kind === 'hora' && has(value, 'uradora_markers')) {
      if (has(value, 'ura_markers')) throw new ProtocolError('Duplicate ura_markers/uradora_markers aliases');
      projection['ura_markers'] = value['uradora_markers'];
    }
    validateDefinition('MjaiEvent', projection);
    if (kind === 'end_game') endReason(value, decimalPaths);
    return value as MjaiEventRecord;
  } catch (cause) {
    throw new ProtocolError(`mjson line ${line} (${kind}): ${cause instanceof Error ? cause.message : String(cause)}`);
  }
}

/**
 * Parse one decompressed UTF-8 mjai event using the Rust-generated known fields.
 * This checks structure, not mahjong legality. Metadata is retained, including
 * finite decimal values; integer tokens must fit JavaScript's exact range.
 */
export function parseMjaiEvent(text: string): MjaiEventRecord { return parseRecord(text, 1); }

function singleLineJson(text: string): string {
  // Do not use trim(): JSON does not permit NBSP, BOM or other Unicode spaces.
  const json = text.replace(/^[\t\n\r ]+|[\t\n\r ]+$/g, '');
  if (/[\r\n]/.test(json)) throw new ProtocolError('A saved JSON record must occupy one physical line');
  return json;
}

function sourceRecord(text: string, line: number): ParsedMjaiRecord {
  const event = parseRecord(text, line);
  try { return { event, json: singleLineJson(text) }; }
  catch (cause) { throw new ProtocolError(`mjson line ${line} (${event.type}): ${cause instanceof Error ? cause.message : String(cause)}`); }
}

/** Keep numeric spellings, aliases, explicit nulls and unknown fields for saving. */
export function parseMjaiRecord(text: string): ParsedMjaiRecord { return sourceRecord(text, 1); }

function annotatedRecord(text: string, line: number): AnnotatedMjaiRecord {
  try {
    const { value, decimalPaths, rawValues } = parseExtensionJson(text, preservedPaths);
    singleLineJson(text);
    if (!isObject(value) || Object.keys(value).length !== annotationFields.size || Object.keys(value).some(key => !annotationFields.has(key))) {
      throw new ProtocolError('An annotated record requires exactly record, schema_version, index, event and annotations');
    }
    if (value['record'] !== 'annotated_mjai' || value['schema_version'] !== 1 || decimalPaths.has('/schema_version')) {
      throw new ProtocolError('Unsupported annotated_mjai record or schema_version');
    }
    const index = value['index'];
    if (typeof index !== 'number' || !Number.isInteger(index) || index < 1 || index > 4_294_967_295 || decimalPaths.has('/index')) {
      throw new ProtocolError('Annotated record index must be an integer token in 1..4294967295');
    }
    const annotations = value['annotations'];
    if (!isObject(annotations)) throw new ProtocolError('annotations must be an object');
    const eventJson = rawValues.get('/event');
    const annotationsJson = rawValues.get('/annotations');
    if (eventJson === undefined || annotationsJson === undefined) throw new ProtocolError('Missing annotated record source text');
    // Use the captured token text. JSON.stringify(value.event) would turn actor:1.0 into actor:1.
    const event = parseRecord(eventJson, line);
    return { record: 'annotated_mjai', schema_version: 1, index, event, annotations, eventJson, annotationsJson };
  } catch (cause) {
    if (cause instanceof ProtocolError && cause.message.startsWith('mjson line ')) throw cause;
    throw new ProtocolError(`mjson line ${line} (annotated_mjai): ${cause instanceof Error ? cause.message : String(cause)}`);
  }
}

/** Validate one annotated record; stream readers additionally check index and lifecycle. */
export function parseAnnotatedMjaiRecord(text: string): AnnotatedMjaiRecord { return annotatedRecord(text, 1); }

/**
 * Validate and embed original event/annotation text without numeric re-serialization.
 * The returned single JSON line has no LF. Its byte limits exclude that absent LF.
 */
export function encodeAnnotatedMjaiRecord(eventJson: string, index: number, annotationsJson: string, limits: LogLimits = {}): string {
  const maximum = resolveLogLimits(limits);
  if (!Number.isInteger(index) || index < 1 || index > 4_294_967_295) throw new ProtocolError('Annotated record index must be in 1..4294967295');
  const event = sourceRecord(eventJson, index);
  const annotations = parseExtensionJson(annotationsJson).value;
  if (!isObject(annotations)) throw new ProtocolError('annotations must be an object');
  const json = `{"record":"annotated_mjai","schema_version":1,"index":${index},"event":${event.json},"annotations":${singleLineJson(annotationsJson)}}`;
  const bytes = encoder.encode(json).length;
  if (bytes > maximum.line) throw new ProtocolError('Output line byte limit exceeded');
  if (bytes > maximum.total) throw new ProtocolError('Output total byte limit exceeded');
  // The wrapper itself must meet the shared depth, node and hard parser limits.
  annotatedRecord(json, index);
  return json;
}

type Phase = 'before' | 'between' | 'playing' | 'outcome' | 'finished';

function nextPhase(phase: Phase, event: MjaiEventRecord, outcome: 'hora' | 'ryukyoku' | null): Phase {
  if (phase === 'finished') throw new ProtocolError('Events after end_game are not permitted');
  switch (event.type) {
    case 'start_game':
      if (phase !== 'before') throw new ProtocolError('Repeated start_game');
      return 'between';
    case 'start_kyoku':
      if (phase !== 'between') throw new ProtocolError('start_kyoku requires start_game or end_kyoku');
      if (!['E', 'S', 'W', 'N'].includes(event.bakaze) || event.kyoku < 1 || event.kyoku > 4 || event.tehais.some(hand => hand.length !== 13)) {
        throw new ProtocolError('start_kyoku requires a wind, kyoku 1..4, and four 13-tile hands');
      }
      return 'playing';
    case 'hora':
      if (phase !== 'playing' && !(phase === 'outcome' && outcome === 'hora')) throw new ProtocolError('hora requires a live round or another hora');
      return 'outcome';
    case 'ryukyoku':
      if (phase !== 'playing') throw new ProtocolError('ryukyoku requires a live round');
      return 'outcome';
    case 'end_kyoku':
      if (phase !== 'outcome') throw new ProtocolError('end_kyoku requires hora or ryukyoku');
      return 'between';
    case 'end_game':
      if (phase === 'before') throw new ProtocolError('end_game requires start_game');
      if (endReason(event) === 'completed' && phase !== 'between') throw new ProtocolError('Completed end_game requires end_kyoku');
      return 'finished';
    case 'none':
      if (phase === 'before') throw new ProtocolError('First event must be start_game');
      return phase;
    default:
      if (phase !== 'playing') throw new ProtocolError('This event requires a live round');
      return phase;
  }
}

/**
 * Bounded structural/lifecycle inspection of decompressed mjson bytes. Yielded
 * records are provisional until clean EOF. Tile flow, turns and settlement are
 * checked by Client.analyzeMjai through the native/WASM Rust implementation.
 */
async function* readValidatedRecords<T extends { readonly event: MjaiEventRecord }>(
  chunks: ByteChunks,
  limits: LogLimits,
  parse: (text: string, line: number) => T,
): AsyncGenerator<T> {
  let line = 0;
  let phase: Phase = 'before';
  let outcome: 'hora' | 'ryukyoku' | null = null;
  try {
    for await (const text of readJsonLines(chunks, limits)) {
      line++;
      const record = parse(text, line);
      const event = record.event;
      try { phase = nextPhase(phase, event, outcome); }
      catch (cause) { throw new ProtocolError(`mjson line ${line} (${event.type}): ${cause instanceof Error ? cause.message : String(cause)}`); }
      if (event.type === 'hora' || event.type === 'ryukyoku') outcome = event.type;
      if (event.type === 'start_kyoku') outcome = null;
      yield record;
    }
  } catch (cause) {
    if (cause instanceof ProtocolError && cause.message.startsWith('mjson line ')) throw cause;
    throw new ProtocolError(`mjson line ${line + 1} (input): ${cause instanceof Error ? cause.message : String(cause)}`);
  }
  if (phase !== 'finished') throw new ProtocolError(`mjson line ${line + 1} (eof): Missing end_game; input is incomplete`);
}

/** Original single-line JSON and typed inspection, provisional until normal EOF. */
export function readMjaiRecords(chunks: ByteChunks, limits: LogLimits = {}): AsyncGenerator<ParsedMjaiRecord> {
  return readValidatedRecords(chunks, limits, sourceRecord);
}

/** Existing event-only inspection. Use readMjaiRecords when saving parsed data. */
export async function* readMjaiEvents(chunks: ByteChunks, limits: LogLimits = {}): AsyncGenerator<MjaiEventRecord> {
  // Preserve the event-only reader's existing JSON-whitespace acceptance.
  // Saving APIs separately require source text that occupies one physical line.
  for await (const record of readValidatedRecords(chunks, limits, (text, line) => ({ event: parseRecord(text, line) }))) yield record.event;
}

/** Read original event and annotation text with consecutive indexes and lifecycle checks. */
export function readAnnotatedMjaiRecords(chunks: ByteChunks, limits: LogLimits = {}): AsyncGenerator<AnnotatedMjaiRecord> {
  return readValidatedRecords(chunks, limits, (text, line) => {
    const record = annotatedRecord(text, line);
    if (record.index !== line) throw new ProtocolError(`mjson line ${line} (annotated_mjai): Non-consecutive record index`);
    return record;
  });
}

function boundedWriter(write: LineWriter, limits: LogLimits): (json: string, line: number) => Promise<void> {
  const maximum = resolveLogLimits(limits);
  let total = 0;
  let count = 0;
  return async (json, line) => {
    const bytes = encoder.encode(json).length;
    if (bytes > maximum.line) throw new ProtocolError(`mjson line ${line} (output): Output line byte limit exceeded`);
    if (++count > maximum.records) throw new ProtocolError(`mjson line ${line} (output): Output record limit exceeded`);
    if (bytes + 1 > maximum.total - total) throw new ProtocolError(`mjson line ${line} (output): Output total byte limit exceeded`);
    total += bytes + 1;
    try { await write(json + '\n'); }
    catch (cause) { throw new ProtocolError(`mjson line ${line} (output): ${cause instanceof Error ? cause.message : String(cause)}`); }
  };
}

/**
 * Wrap a complete mjai stream with empty annotations. All writes remain provisional
 * until this promise resolves after normal EOF; the caller owns atomic publication.
 */
export async function wrapMjaiRecords(chunks: ByteChunks, write: LineWriter, limits: LogLimits = {}): Promise<MjaiTranscodeReport> {
  const output = boundedWriter(write, limits);
  let records = 0;
  for await (const record of readMjaiRecords(chunks, limits)) {
    records++;
    await output(encodeAnnotatedMjaiRecord(record.json, records, '{}', limits), records);
  }
  return { records, droppedAnnotationRecords: 0 };
}

/** Explicitly opt into annotation removal; the resolved report counts affected rows. */
export async function unwrapAnnotatedMjaiRecords(
  chunks: ByteChunks,
  write: LineWriter,
  options: UnwrapMjaiOptions = {},
  limits: LogLimits = {},
): Promise<MjaiTranscodeReport> {
  if (!isObject(options) || Object.keys(options).some(key => key !== 'dropAnnotations')) {
    throw new ProtocolError('dropAnnotations must be an explicit boolean');
  }
  // Authorize once, before an iterator or writer can mutate the caller's object.
  // Inherited properties do not grant permission; an own getter is read once.
  const hasDropOption = has(options, 'dropAnnotations');
  const dropOption = hasDropOption ? options.dropAnnotations : undefined;
  if (hasDropOption && typeof dropOption !== 'boolean') throw new ProtocolError('dropAnnotations must be an explicit boolean');
  const dropAnnotations = dropOption === true;
  const output = boundedWriter(write, limits);
  let records = 0;
  let droppedAnnotationRecords = 0;
  for await (const record of readAnnotatedMjaiRecords(chunks, limits)) {
    if (Object.keys(record.annotations).length !== 0) {
      if (!dropAnnotations) throw new ProtocolError(`mjson line ${record.index} (annotations): Refusing to discard nonempty annotations`);
      droppedAnnotationRecords++;
    }
    records++;
    await output(record.eventJson, record.index);
  }
  return { records, droppedAnnotationRecords };
}
