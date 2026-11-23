import { Engine } from '../wasm/node/suzume_wasm.js';
import { Client } from './client.js';
export * from './index.js';

/** Node's generated binding loads the packaged WASM synchronously. */
export function createNodeEngine(): Client {
  const engine = new Engine();
  return new Client({ request: input => engine.request(input), close: () => engine.free() });
}
