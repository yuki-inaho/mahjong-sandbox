import { createBrowserEngine } from '../../packages/suzume/dist/browser.js';

const byId = id => {
  const element = document.getElementById(id);
  if (!element) throw new Error(`必要な画面要素がありません: ${id}`);
  return element;
};
let client;
let state;
let busy = false;
window.__appReady = false;
const honors = ['東', '南', '西', '北', '白', '發', '中'];
const tileName = id => id < 27 ? `${id % 9 + 1}${['萬', '筒', '索'][Math.floor(id / 9)]}` : honors[id - 27];
const tileElement = (id, button = false) => {
  const element = document.createElement(button ? 'button' : 'span');
  element.className = `tile suit-${Math.floor(id / 9)}`;
  element.textContent = tileName(id);
  return element;
};

function render() {
  if (!state) return;
  byId('round').textContent = state.phase.kind === 'match_end' ? '対局終了' : `東${state.round + 1}局 · ${state.honba}本場`;
  byId('wall').textContent = `残り${state.wall_remaining}枚 / 供託${state.riichi_pot}点`;
  byId('dora').replaceChildren(...state.dora_indicators.map(tile => tileElement(tile)));
  byId('players').replaceChildren(...state.players.map(player => {
    const panel = document.createElement('section');
    panel.className = `player${state.actor === player.seat ? ' active' : ''}`;
    const heading = document.createElement('h3');
    heading.textContent = `${player.seat === 0 ? 'あなた' : `CPU ${player.seat}`} ${state.dealer === player.seat ? '親' : ''}${player.riichi ? ' · リーチ' : ''}`;
    const score = document.createElement('p'); score.className = 'score'; score.textContent = `${player.points.toLocaleString('ja-JP')} 点`;
    const discards = document.createElement('div'); discards.className = 'river';
    for (const discard of player.discards) {
      const tile = tileElement(discard.tile); tile.classList.toggle('called', discard.called); discards.append(tile);
    }
    const melds = document.createElement('p'); melds.className = 'melds';
    melds.textContent = player.melds.map(meld => meld.tiles.map(tileName).join(' ')).join(' / ') || '副露なし';
    panel.append(heading, score, discards, melds); return panel;
  }));
  byId('hand').replaceChildren(...(state.own_hand ?? []).map((tile, index) => {
    const button = tileElement(tile, true);
    button.setAttribute('aria-label', `${index + 1}枚目 ${tileName(tile)}を打牌`);
    const legal = state.legal_actions.find(item => item.action.kind === 'discard' && item.action.index === index);
    button.disabled = busy || !legal;
    if (legal) button.addEventListener('click', () => operate(async () => { state = await client.apply(legal.action); }));
    return button;
  }));
  byId('actions').replaceChildren(...state.legal_actions.filter(item => item.action.kind !== 'discard').map(item => {
    const button = document.createElement('button'); button.textContent = item.label; button.disabled = busy;
    button.addEventListener('click', () => operate(async () => { state = await client.apply(item.action); })); return button;
  }));
  const ended = state.phase.kind === 'match_end';
  const human = state.actor === 0;
  byId('turn').textContent = ended ? '最終得点を確認できます。' : human ? (state.phase.kind === 'round_end' ? '局の結果を確認して進めてください。' : 'あなたの操作を選んでください。') : `CPU ${state.actor}の操作待ちです。`;
  byId('status').textContent = `対局 ${state.session_id} · ${state.revision} 操作${busy ? ' · 処理中' : ''}`;
  for (const id of ['advance', 'one-step']) byId(id).disabled = busy || human || ended;
  byId('new-game').disabled = busy;
  byId('seed').disabled = busy; byId('preset').disabled = busy;
}

async function operate(operation) {
  if (busy) return;
  busy = true; byId('error').hidden = true; render();
  try { await operation(); }
  catch (error) { byId('error').textContent = error instanceof Error ? error.message : String(error); byId('error').hidden = false; }
  finally { busy = false; render(); }
}
async function start() {
  const preset = byId('preset').value;
  state = await client.newGame({ schema_version: 1, game: { seed: byId('seed').value, viewer: 0 }, players: [
    { kind: 'human' }, ...[1, 2, 3].map(seat => ({ kind: 'ai', config: { preset, seed: String(seat) } })),
  ] });
}
byId('new-game').addEventListener('click', () => operate(start));
byId('one-step').addEventListener('click', () => operate(async () => { state = await client.stepBot(); }));
byId('advance').addEventListener('click', () => operate(async () => {
  for (let count = 0; count < 256 && state.actor !== 0 && state.phase.kind !== 'match_end'; count++) {
    state = await client.stepBot();
    if (count % 8 === 0) { render(); await new Promise(resolve => requestAnimationFrame(resolve)); }
  }
}));
window.addEventListener('pagehide', () => client?.close());
// Read-only observation for the reproducible browser acceptance check.
window.__suzumeView = () => state ? structuredClone(state) : null;
try { client = await createBrowserEngine(); await operate(start); window.__appReady = Boolean(state); }
catch (error) { byId('error').hidden = false; byId('error').textContent = `対局を開始できません: ${String(error)}`; }
