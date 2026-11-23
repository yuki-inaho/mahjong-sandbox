import init, { Engine } from '../wasm/web/suzume_wasm.js';
import { Client } from './client.js';
export * from './index.js';

/** A URL uses fetch; supplied bytes/modules also support offline embedded applications. */
export async function createBrowserEngine(source?: URL | BufferSource | WebAssembly.Module): Promise<Client> {
  if (source === undefined) await init();
  else await init({ module_or_path: source });
  const engine = new Engine();
  return new Client({ request: input => engine.request(input), close: () => engine.free() });
}
