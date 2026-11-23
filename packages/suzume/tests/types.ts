import type { Request, WireAction, GameView, EnvironmentTransition } from '../src/index.js';
import { Client } from '../src/index.js';

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

async function consumer(client: Client): Promise<void> {
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
