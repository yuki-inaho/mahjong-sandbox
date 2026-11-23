import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { createNodeEngine } from '../dist/node.js';
import { createBrowserEngine } from '../dist/browser.js';
import { Engine } from '../wasm/node/suzume_wasm.js';
import { ApiFault, ProtocolError, parseResponse } from '../dist/index.js';

const root = fileURLToPath(new URL('../../../', import.meta.url));

test('native Rust and real Node WASM produce identical versioned responses', () => {
  const wasm = new Engine();
  const requests = [];
  const expected = [];
  const call = input => {
    const text = typeof input === 'string' ? input : JSON.stringify(input);
    requests.push(text);
    const response = JSON.parse(wasm.request(text)); expected.push(response);
    parseResponse(JSON.stringify(response));
    return response;
  };
  try {
    call('{'); call('{"version":1,"version":1,"op":"view"}');
    call({ version: 2, op: 'new' });
    let response = call({ version: 1, op: 'new', config: { schema_version: 1, game: { seed: '18446744073709551615' } } });
    for (let i = 0; i < 64; i++) {
      assert.equal(response.status, 'ok');
      const state = response.data.state;
      const stamp = { version: 1, session_id: state.session_id, expected_revision: state.revision };
      const first = state.legal_actions[0];
      response = call(first ? { ...stamp, op: 'apply', action: first.action } : { ...stamp, op: 'step' });
    }
    call({ version: 1, op: 'step', session_id: 1, expected_revision: 0 });
    call({ version: 1, op: 'new' });
    call({ version: 1, op: 'analyze', session_id: 2, expected_revision: 0, config: { preset: 'strong', seed: '99' } });
    call({ version: 1, op: 'inspect_config', toml: 'schema_version = 1\n[game]\nseed = "9007199254740993"\n' });
    call({ version: 1, op: 'load_config', toml: 'schema_version = 1\nunknown = true\n' });
    response = call({ version: 1, op: 'environment_reset', seed: '42', max_steps: 8 });
    for (let i = 0; i < 8; i++) {
      const state = response.data.transition;
      response = call({ version: 1, op: 'environment_step', session_id: state.session_id, expected_revision: state.revision, action_id: state.observation.legal_mask.findIndex(Boolean) });
    }
    assert.equal(response.data.transition.truncated, true);
    const native = spawnSync('cargo', ['run', '--quiet', '--locked', '-p', 'suzume-api', '--bin', 'suzume-api', '--', 'serve'], {
      cwd: root, input: requests.join('\n') + '\n', encoding: 'utf8', timeout: 120_000, maxBuffer: 8_388_608,
    });
    assert.equal(native.error, undefined);
    assert.equal(native.status, 0, native.stderr);
    assert.deepEqual(native.stdout.trimEnd().split('\n').map(line => JSON.parse(line)), expected);
  } finally { wasm.free(); }
});

test('typed client checks stale actions, private views and terminal masks using real WASM', async () => {
  const client = createNodeEngine();
  try {
    const initial = await client.newGame();
    assert.equal(initial.own_hand.length, 14);
    assert.equal(initial.players.some(player => 'hand' in player), false);
    assert.equal('seed' in initial, false);
    const first = initial.legal_actions[0]; assert.ok(first);
    const after = await client.apply(first.action);
    const stale = await client.command({ version: 1, op: 'apply', session_id: initial.session_id, expected_revision: initial.revision, action: first.action });
    assert.equal(stale.status, 'error'); assert.equal(stale.code, 'stale_revision');
    assert.equal((await client.view()).revision, after.revision);
    let state = await client.environmentReset('18446744073709551615', 3);
    for (let i = 0; i < 3; i++) {
      assert.equal(state.observation.legal_mask.length, 189);
      state = await client.environmentStep(state.observation.legal_mask.findIndex(Boolean));
      assert.equal(state.rewards.reduce((a, b) => a + b, 0) + state.escrow_delta, 0);
    }
    assert.equal(state.truncated, true); assert.equal(state.observation, null);
    assert.ok(state.final_observation); assert.equal(state.final_observation.legal_mask.length, 189);
    await assert.rejects(client.environmentStep(72), error => error instanceof ApiFault && error.code === 'step_limit');
  } finally { client.close(); }
  await assert.rejects(client.newGame(), ProtocolError);
});

test('browser ESM binding instantiates packaged bytes without Node-specific browser code', async () => {
  const bytes = readFileSync(new URL('../wasm/web/suzume_wasm_bg.wasm', import.meta.url));
  const client = await createBrowserEngine(bytes);
  try {
    const result = await client.newGame();
    assert.equal(result.revision, 0); assert.equal(result.own_hand.length, 14);
    assert.equal((await client.inspectConfig('schema_version = 1')).effective_ai[1].algorithm, 'greedy');
  } finally { client.close(); }
});

test('mjson analysis uses the same real Node/Web WASM and native semantics without mutating sessions', async () => {
  const hands = Array.from({ length: 4 }, () => Array(13).fill('?'));
  hands[1][0] = '9s';
  const events = [
    { type: 'start_game' },
    { type: 'start_kyoku', bakaze: 'E', dora_marker: '1p', kyoku: 1, honba: 0, kyotaku: 0,
      oya: 0, scores: [25000, 25000, 25000, 25000], tehais: hands },
    { type: 'tsumo', actor: 0, pai: '5mr' },
    { type: 'dahai', actor: 0, pai: '5mr', tsumogiri: true },
    { type: 'ryukyoku', deltas: [0, 0, 0, 0] }, { type: 'end_kyoku' }, { type: 'end_game' },
  ];
  const mjson = events.map(event => JSON.stringify(event)).join('\n');
  const bytes = readFileSync(new URL('../wasm/web/suzume_wasm_bg.wasm', import.meta.url));
  const node = createNodeEngine();
  const web = await createBrowserEngine(bytes);
  try {
    const before = await node.newGame();
    const result = await node.analyzeMjai(mjson, 0);
    assert.equal(result.summary.records, 7);
    assert.equal(result.summary.red_events, 2);
    assert.equal(result.summary.complete, true);
    assert.equal(result.observation.record_ended, true);
    assert.equal(result.observation.game_ended, true);
    assert.equal(result.observation.own_hand.length, 13);
    assert.equal(JSON.stringify(result).includes('9s'), false);
    assert.deepEqual(await node.view(), before);
    assert.deepEqual(await web.analyzeMjai(mjson, 0), result);
    const publicResult = await node.analyzeMjai(mjson);
    assert.equal(publicResult.observation.own_hand, null);
    assert.equal(publicResult.observation.viewer, null);
    const interrupted = [...events.slice(0, 3), { type: 'end_game', _suzume: { reason: 'interrupted', complete: false } }];
    const cutoff = await node.analyzeMjai(interrupted.map(event => JSON.stringify(event)).join('\n'), 0);
    assert.equal(cutoff.summary.complete, false);
    assert.equal(cutoff.observation.record_ended, true);
    assert.equal(cutoff.observation.game_ended, false);
    assert.equal(cutoff.observation.round_ended, false);
    const environment = await node.environmentReset('17', 10);
    await node.analyzeMjai(mjson);
    assert.deepEqual(await node.environmentView(), environment);
    const broken = [...events]; broken[2] = { ...events[2], actor: 2 };
    await assert.rejects(node.analyzeMjai(broken.map(event => JSON.stringify(event)).join('\n')),
      error => error instanceof ApiFault && error.code === 'log_error' && /line 3 \(tsumo\)/.test(error.message));
    assert.deepEqual(await node.environmentView(), environment);
    const request = { version: 1, op: 'analyze_mjai', mjson, viewer: 0 };
    const native = spawnSync('cargo', ['run', '--quiet', '--locked', '-p', 'suzume-api', '--bin', 'suzume-api', '--', 'serve'], {
      cwd: root, input: JSON.stringify(request) + '\n', encoding: 'utf8', timeout: 120_000,
    });
    assert.equal(native.error, undefined); assert.equal(native.status, 0, native.stderr);
    const nativeResult = parseResponse(native.stdout);
    assert.equal(nativeResult.status, 'ok'); assert.deepEqual(nativeResult.data, result);
  } finally { node.close(); web.close(); }
});
