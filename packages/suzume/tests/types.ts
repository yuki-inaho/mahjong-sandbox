import type { Request, WireAction, GameView, EnvironmentTransition, MjaiEvent, MjaiEventRecord } from '../src/index.js';
import { Client, parseMjaiEvent } from '../src/index.js';

const valid: Request = { version: 1, op: 'environment_reset', seed: '18446744073709551615', max_steps: 1000 };
const chi: WireAction = { kind: 'chi', tiles: [0, 1] };
void valid; void chi;
// @ts-expect-error The protocol version is a literal, not an arbitrary number.
const wrongVersion: Request = { version: 2, op: 'view' };
// @ts-expect-error Unknown operations cannot cross the typed boundary.
const unknownOperation: Request = { version: 1, op: 'set_wall' };
// @ts-expect-error A 64-bit seed is exact decimal text, not a JavaScript number.
const lossySeed: Request = { version: 1, op: 'environment_reset', seed: 42 };
// @ts-expect-error Chi has precisely two consumed tiles.
const badChi: WireAction = { kind: 'chi', tiles: [0] };
// @ts-expect-error Discard indexes and tile-based environment actions are separate shapes.
const mixedAction: WireAction = { kind: 'discard', tile: 3 };
void wrongVersion; void unknownOperation; void lossySeed; void badChi; void mixedAction;

const analyzeRecord: Request = { version: 1, op: 'analyze_mjai', mjson: '', viewer: 0 };
const event: MjaiEvent = { type: 'chi', actor: 1, target: 0, pai: '1m', consumed: ['2m', '3m'] };
const raw: MjaiEventRecord = parseMjaiEvent('{"type":"none","meta":{"confidence":0.25}}');
const metadata: unknown = raw['meta'];
// @ts-expect-error mjai text is not an arbitrary JSON value.
const wrongMjson: Request = { version: 1, op: 'analyze_mjai', mjson: {} };
// @ts-expect-error A seat number cannot be replaced by a name.
const wrongViewer: Request = { version: 1, op: 'analyze_mjai', mjson: '', viewer: 'east' };
// @ts-expect-error Chi's two consumed tiles are a generated fixed tuple.
const shortMeld: MjaiEvent = { type: 'chi', actor: 1, target: 0, pai: '1m', consumed: ['2m'] };
// @ts-expect-error Uninterpreted producer metadata stays unknown, never any.
const uncheckedMetadata: number = raw['meta'];
void analyzeRecord; void event; void metadata; void wrongMjson; void wrongViewer; void shortMeld; void uncheckedMetadata;

async function consumer(client: Client): Promise<void> {
  const analysis = await client.analyzeMjai('{"type":"start_game"}\n{"type":"end_game"}');
  const score: number | undefined = analysis.summary.final_scores?.[0];
  void score;
  const game: GameView = await client.newGame();
  const action = game.legal_actions[0];
  if (action) await client.apply(action.action);
  const environment: EnvironmentTransition = await client.environmentReset('42', 10);
  const mask = environment.observation?.legal_mask;
  if (mask) {
    const id = mask.findIndex(Boolean);
    if (id >= 0) await client.environmentStep(id);
  }
}
void consumer;
