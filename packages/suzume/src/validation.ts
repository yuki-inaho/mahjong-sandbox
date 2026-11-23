import { protocolSchema } from './generated/schema.js';
import type { Request, Response } from './generated/types.js';

export class ProtocolError extends Error {
  constructor(message: string) { super(message); this.name = 'ProtocolError'; }
}

type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
type Schema = {
  readonly $ref?: string;
  readonly type?: string | readonly string[];
  readonly enum?: readonly Json[];
  readonly oneOf?: readonly Schema[];
  readonly anyOf?: readonly Schema[];
  readonly allOf?: readonly Schema[];
  readonly properties?: Readonly<Record<string, Schema>>;
  readonly required?: readonly string[];
  readonly additionalProperties?: boolean;
  readonly items?: Schema;
  readonly minItems?: number;
  readonly maxItems?: number;
  readonly minimum?: number;
  readonly maximum?: number;
  readonly pattern?: string;
  readonly format?: string;
  readonly minLength?: number;
  readonly maxLength?: number;
};
const definitions = (protocolSchema as { definitions: Readonly<Record<string, Schema>> }).definitions;
const LIMIT = 1_048_576;
const MAX_NODES = 50_000;
const own = (value: object, key: string): boolean => Object.prototype.hasOwnProperty.call(value, key);
const invalidSurrogate = /[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/u;

/** Strict protocol JSON: integer tokens only; no duplicate keys or lone surrogates. */
export function parseJson(text: string): unknown {
  if (text.length > LIMIT || new TextEncoder().encode(text).length > LIMIT) throw new ProtocolError('JSON exceeds 1048576 bytes');
  let cursor = 0;
  let nodes = 0;
  const fail = (): never => { throw new ProtocolError(`Invalid JSON at character ${cursor}`); };
  const space = (): void => { while (/^[\t\n\r ]$/.test(text[cursor] ?? '')) cursor++; };
  const quoted = (): string => {
    const start = cursor++;
    while (cursor < text.length) {
      const character = text[cursor++];
      if (character === '\\') cursor++;
      else if (character === '"') {
        let result: unknown;
        try { result = JSON.parse(text.slice(start, cursor)); } catch { return fail(); }
        if (typeof result !== 'string' || invalidSurrogate.test(result)) return fail();
        return result;
      }
    }
    return fail();
  };
  const value = (depth: number): Json => {
    if (depth > 64 || ++nodes > MAX_NODES) throw new ProtocolError('JSON structure limit exceeded');
    space();
    if (text[cursor] === '"') return quoted();
    if (text[cursor] === '{') {
      cursor++; space();
      const result: { [key: string]: Json } = Object.create(null) as { [key: string]: Json };
      if (text[cursor] === '}') { cursor++; return result; }
      while (true) {
        space(); if (text[cursor] !== '"') return fail();
        const key = quoted();
        if (own(result, key)) throw new ProtocolError(`Duplicate JSON property: ${key}`);
        space(); if (text[cursor++] !== ':') return fail();
        result[key] = value(depth + 1); space();
        if (text[cursor] === '}') { cursor++; return result; }
        if (text[cursor++] !== ',') return fail();
      }
    }
    if (text[cursor] === '[') {
      cursor++; space();
      const result: Json[] = [];
      if (text[cursor] === ']') { cursor++; return result; }
      while (true) {
        result.push(value(depth + 1)); space();
        if (text[cursor] === ']') { cursor++; return result; }
        if (text[cursor++] !== ',') return fail();
      }
    }
    for (const [word, result] of [['null', null], ['true', true], ['false', false]] as const) {
      if (text.startsWith(word, cursor)) { cursor += word.length; return result; }
    }
    const token = /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/.exec(text.slice(cursor));
    if (!token) return fail();
    cursor += token[0].length;
    if (token[0] === '-0' || /[.eE]/.test(token[0])) throw new ProtocolError('Integer wire fields do not accept negative zero, decimal or exponent notation');
    const result = Number(token[0]);
    if (!Number.isSafeInteger(result)) return fail();
    return result;
  };
  const result = value(0); space();
  if (cursor !== text.length) return fail();
  return result;
}

function checkStructure(value: unknown): void {
  let count = 0;
  const active = new Set<object>();
  const visit = (item: unknown, depth: number): void => {
    if (++count > MAX_NODES || depth > 64) throw new ProtocolError('Value structure limit exceeded');
    if (item === null || typeof item === 'boolean') return;
    if (typeof item === 'string') {
      if (item.length > LIMIT || invalidSurrogate.test(item)) throw new ProtocolError('Invalid string');
      return;
    }
    if (typeof item === 'number') {
      if (!Number.isFinite(item)) throw new ProtocolError('Non-finite number');
      return;
    }
    if (typeof item !== 'object' || active.has(item)) throw new ProtocolError('Value is not a finite JSON tree');
    const prototype: unknown = Object.getPrototypeOf(item);
    if (!Array.isArray(item) && prototype !== Object.prototype && prototype !== null) throw new ProtocolError('Expected a plain object');
    active.add(item);
    for (const key of Reflect.ownKeys(item)) {
      if (Array.isArray(item) && key === 'length') continue;
      if (typeof key !== 'string') throw new ProtocolError('Symbol properties are not JSON');
      const descriptor = Object.getOwnPropertyDescriptor(item, key);
      if (!descriptor || !('value' in descriptor) || !descriptor.enumerable) throw new ProtocolError('Accessors and hidden properties are not JSON');
      visit(descriptor.value, depth + 1);
    }
    if (Array.isArray(item)) {
      const keys = Object.keys(item);
      if (keys.length !== item.length || keys.some((key, index) => key !== String(index))) throw new ProtocolError('Sparse arrays and extra array properties are not JSON arrays');
    }
    active.delete(item);
  };
  visit(value, 0);
}

const numericFormats: Readonly<Record<string, readonly [number, number]>> = {
  uint8: [0, 255], uint16: [0, 65_535], uint32: [0, 4_294_967_295],
  int8: [-128, 127], int16: [-32_768, 32_767], int32: [-2_147_483_648, 2_147_483_647],
};
function matches(schema: Schema, value: unknown): boolean {
  if (schema.$ref) {
    const name = schema.$ref.replace('#/definitions/', '');
    const target = own(definitions, name) ? definitions[name] : undefined;
    if (!target) throw new ProtocolError('Unresolved generated schema reference');
    return matches(target, value);
  }
  if (schema.oneOf && schema.oneOf.filter(part => matches(part, value)).length !== 1) return false;
  if (schema.anyOf && !schema.anyOf.some(part => matches(part, value))) return false;
  if (schema.allOf && !schema.allOf.every(part => matches(part, value))) return false;
  if (schema.enum && !schema.enum.some(part => part === value)) return false;
  const allowed = typeof schema.type === 'string' ? [schema.type] : schema.type;
  const kind = value === null ? 'null' : Array.isArray(value) ? 'array' : typeof value === 'number' && Number.isInteger(value) ? 'integer' : typeof value;
  if (allowed && !allowed.includes(kind)) return false;
  if (typeof value === 'number') {
    if (!Number.isSafeInteger(value)) return false;
    if (schema.minimum !== undefined && value < schema.minimum) return false;
    if (schema.maximum !== undefined && value > schema.maximum) return false;
    const range = schema.format ? numericFormats[schema.format] : undefined;
    if (range && (value < range[0] || value > range[1])) return false;
  }
  if (typeof value === 'string') {
    if (schema.pattern && !new RegExp(schema.pattern, 'u').test(value)) return false;
    if (schema.minLength !== undefined && [...value].length < schema.minLength) return false;
    if (schema.maxLength !== undefined && [...value].length > schema.maxLength) return false;
    if (schema.format === 'uint64-decimal' && (!/^(0|[1-9][0-9]{0,19})$/.test(value) || BigInt(value) > 18_446_744_073_709_551_615n)) return false;
    if (schema.format === 'int64-decimal' && (!/^(0|-?[1-9][0-9]{0,18})$/.test(value) || BigInt(value) < -9_223_372_036_854_775_808n || BigInt(value) > 9_223_372_036_854_775_807n)) return false;
  }
  if (Array.isArray(value)) {
    if (schema.minItems !== undefined && value.length < schema.minItems) return false;
    if (schema.maxItems !== undefined && value.length > schema.maxItems) return false;
    if (schema.items && !value.every(item => matches(schema.items as Schema, item))) return false;
  } else if (value !== null && typeof value === 'object') {
    const object = value as Record<string, unknown>;
    if (schema.required?.some(key => !own(object, key))) return false;
    if (schema.properties) {
      for (const key of Object.keys(object)) {
        const property = own(schema.properties, key) ? schema.properties[key] : undefined;
        if (property ? !matches(property, object[key]) : schema.additionalProperties === false) return false;
      }
    }
  }
  return true;
}

export function validateDefinition(name: string, value: unknown): void {
  const schema = own(definitions, name) ? definitions[name] : undefined;
  if (!schema) throw new ProtocolError(`Unknown schema definition: ${name}`);
  checkStructure(value);
  if (!matches(schema, value)) throw new ProtocolError(`Value does not conform to ${name}`);
}
export function assertRequest(value: unknown): asserts value is Request { validateDefinition('Request', value); }
export function parseResponse(text: string): Response {
  const value = parseJson(text);
  validateDefinition('Response', value);
  return value as Response;
}
