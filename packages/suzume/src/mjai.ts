import type { MjaiEvent } from './generated/types.js';
import { protocolSchema } from './generated/schema.js';
import { readJsonLines, type LogLimits } from './logs.js';
import { parseExtensionJson, ProtocolError, validateDefinition } from './validation.js';

/** Known fields have generated types; uninterpreted producer extensions stay unknown. */
export type MjaiEventRecord = MjaiEvent & Readonly<Record<string, unknown>>;

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
export async function* readMjaiEvents(chunks: AsyncIterable<Uint8Array> | Iterable<Uint8Array>, limits: LogLimits = {}): AsyncGenerator<MjaiEventRecord> {
  let line = 0;
  let phase: Phase = 'before';
  let outcome: 'hora' | 'ryukyoku' | null = null;
  try {
    for await (const text of readJsonLines(chunks, limits)) {
      line++;
      const event = parseRecord(text, line);
      try { phase = nextPhase(phase, event, outcome); }
      catch (cause) { throw new ProtocolError(`mjson line ${line} (${event.type}): ${cause instanceof Error ? cause.message : String(cause)}`); }
      if (event.type === 'hora' || event.type === 'ryukyoku') outcome = event.type;
      if (event.type === 'start_kyoku') outcome = null;
      yield event;
    }
  } catch (cause) {
    if (cause instanceof ProtocolError && cause.message.startsWith('mjson line ')) throw cause;
    throw new ProtocolError(`mjson line ${line + 1} (input): ${cause instanceof Error ? cause.message : String(cause)}`);
  }
  if (phase !== 'finished') throw new ProtocolError(`mjson line ${line + 1} (eof): Missing end_game; input is incomplete`);
}
