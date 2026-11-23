import type { DatasetTransition, GameView, LogRecord } from './generated/types.js';
import { parseJson, ProtocolError, validateDefinition } from './validation.js';

export interface LogLimits {
  readonly maxLineBytes?: number;
  readonly maxRecords?: number;
  readonly maxTotalBytes?: number;
}

/** Internal shared limits for input and output transports. */
export function resolveLogLimits(limits: LogLimits): { readonly line: number; readonly records: number; readonly total: number } {
  const maximum = { line: limits.maxLineBytes ?? 1_048_576, records: limits.maxRecords ?? 10_002, total: limits.maxTotalBytes ?? 268_435_456 };
  for (const limit of Object.values(maximum)) if (!Number.isSafeInteger(limit) || limit <= 0) throw new ProtocolError('Log limits must be positive safe integers');
  return maximum;
}

/** A record parser validates structure only. It does not replay mahjong rules. */
export function parseLogRecord(text: string): LogRecord {
  const value = parseJson(text); validateDefinition('LogRecord', value); return value as LogRecord;
}
export function parseTransition(text: string): DatasetTransition {
  const value = parseJson(text); validateDefinition('DatasetTransition', value); return value as DatasetTransition;
}

/** Shared bounded UTF-8 JSON-lines transport. Consumers define record semantics. */
export async function* readJsonLines(chunks: AsyncIterable<Uint8Array> | Iterable<Uint8Array>, limits: LogLimits): AsyncGenerator<string> {
  const maximum = resolveLogLimits(limits);
  // The shared record parser has its own 1 MiB cap. A higher line value cannot disable it.
  const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
  const encoder = new TextEncoder();
  let buffered = '';
  let total = 0;
  let count = 0;
  const checked = (line: string): string => {
    if (encoder.encode(line).length > maximum.line) throw new ProtocolError('Log line limit exceeded');
    if (++count > maximum.records) throw new ProtocolError('Log record limit exceeded');
    if (line.trim().length === 0) throw new ProtocolError('Empty log records are not permitted');
    return line;
  };
  for await (const chunk of chunks) {
    total += chunk.byteLength;
    if (total > maximum.total) throw new ProtocolError('Log total byte limit exceeded');
    // Small decoding slices bound the carry buffer even when a caller supplies a huge chunk.
    for (let offset = 0; offset < chunk.byteLength; offset += 16_384) {
      try { buffered += decoder.decode(chunk.subarray(offset, offset + 16_384), { stream: true }); }
      catch { throw new ProtocolError('Log is not UTF-8'); }
      let newline: number;
      while ((newline = buffered.indexOf('\n')) >= 0) {
        const line = buffered.slice(0, newline); buffered = buffered.slice(newline + 1);
        yield checked(line);
      }
      if (encoder.encode(buffered).length > maximum.line) throw new ProtocolError('Log line limit exceeded');
    }
  }
  try { buffered += decoder.decode(); } catch { throw new ProtocolError('Incomplete UTF-8 at log end'); }
  if (buffered.length > 0) yield checked(buffered);
}

function requirePublic(view: GameView): void {
  if (view.viewer != null || view.own_hand != null || view.own_last_draw != null || view.own_furiten != null || view.legal_actions.length !== 0) {
    throw new ProtocolError('A public log contains private observation fields');
  }
}

/**
 * Structural streaming inspection: validates record types, ordering and completion.
 * Yielded records are provisional until clean EOF; Rust replay additionally checks
 * engine/rule compatibility, hashes and every legal state transition.
 */
export async function* readLogRecords(chunks: AsyncIterable<Uint8Array> | Iterable<Uint8Array>, limits: LogLimits = {}): AsyncGenerator<LogRecord> {
  let mode: 'replay' | 'public' | null = null;
  let sequence = 0;
  let maximum = 0;
  let ended = false;
  for await (const line of readJsonLines(chunks, limits)) {
    const record = parseLogRecord(line);
    if (ended) throw new ProtocolError('Records after the footer are not permitted');
    if (mode === null) {
      if (record.record === 'replay_header') {
        mode = 'replay'; maximum = record.config.game?.max_steps ?? 10_000;
      } else if (record.record === 'public_header') {
        mode = 'public'; maximum = record.max_steps; requirePublic(record.initial);
      } else throw new ProtocolError('The first record must be a header');
    } else if ((mode === 'replay' && record.record === 'replay_action') || (mode === 'public' && record.record === 'public_step')) {
      sequence++;
      if (record.sequence !== sequence || record.revision !== sequence || sequence > maximum) throw new ProtocolError('Non-consecutive or excessive action sequence');
      if (record.record === 'public_step') requirePublic(record.view);
    } else if ((mode === 'replay' && record.record === 'replay_footer') || (mode === 'public' && record.record === 'public_footer')) {
      if (record.action_count !== sequence || record.revision !== sequence) throw new ProtocolError('Footer count does not match the stream');
      if (record.record === 'public_footer') requirePublic(record.final_view);
      ended = true;
    } else throw new ProtocolError('Mixed log formats or unexpected record kind');
    yield record;
  }
  if (!ended) throw new ProtocolError('Missing footer; the log is incomplete');
}

/** Reads exported dataset rows; partial iteration is not complete-file validation. */
export async function* readTransitions(chunks: AsyncIterable<Uint8Array> | Iterable<Uint8Array>, limits: LogLimits = {}): AsyncGenerator<DatasetTransition> {
  let sequence = 0;
  let ended = false;
  for await (const line of readJsonLines(chunks, limits)) {
    const row = parseTransition(line);
    if (ended || row.sequence !== ++sequence) throw new ProtocolError('Invalid dataset sequence');
    if (row.actor !== row.observation.actor || row.observation.legal_mask[row.action_id] !== true) throw new ProtocolError('Action does not match the actor observation');
    if (row.rewards.reduce((sum, reward) => sum + reward, 0) + row.escrow_delta !== 0) throw new ProtocolError('Dataset rewards do not conserve points and escrow');
    if (row.terminated && row.truncated) throw new ProtocolError('Termination and truncation are mutually exclusive');
    ended = row.terminated || row.truncated;
    if (ended && (row.next_actor != null || row.next_observation != null)) throw new ProtocolError('Finished transitions must not contain a next observation');
    if (row.truncated ? row.final_observation == null : row.final_observation != null) throw new ProtocolError('Only truncated transitions must retain their final observation');
    if (!ended && (row.next_actor == null || row.next_observation == null || row.next_actor !== row.next_observation.actor)) throw new ProtocolError('Continuing transition has no matching next actor');
    yield row;
  }
  if (sequence > 0 && !ended) throw new ProtocolError('Dataset ended before a terminal or truncated row');
}
