import type { ActionId, AiConfigSpec, AppConfig, DecisionView, EnvironmentTransition, ErrorCode, GameView, MjaiFormat, Request, Response, ResponseData, SeatId, Seed, WireAction } from './generated/types.js';
import { assertRequest, parseResponse, ProtocolError } from './validation.js';

export interface Transport {
  request(input: string): string | Promise<string>;
  close?(): void;
}

export class ApiFault extends Error {
  readonly code: ErrorCode;
  readonly sessionId: number | null;
  readonly revision: number | null;
  constructor(response: Extract<Response, { status: 'error' }>) {
    super(response.message); this.name = 'ApiFault'; this.code = response.code;
    this.sessionId = response.session_id ?? null; this.revision = response.revision ?? null;
  }
}

type Stamp = { readonly session_id: number; readonly expected_revision: number };

/** A local engine client. Responses are validated before entering application code. */
export class Client {
  private state: GameView | EnvironmentTransition | null = null;
  private closed = false;
  private pending: Promise<void> = Promise.resolve();
  constructor(private readonly transport: Transport) {}

  /** Raw typed protocol, including structured errors; operations run in arrival order. */
  command(request: Request): Promise<Response> {
    const run = async (): Promise<Response> => {
      if (this.closed) throw new ProtocolError('Client is closed');
      assertRequest(request);
      const text = JSON.stringify(request);
      if (new TextEncoder().encode(text).length > 1_048_576) throw new ProtocolError('Request exceeds 1048576 bytes');
      const response = parseResponse(await this.transport.request(text));
      if (response.status === 'ok') {
        if (response.data.kind === 'game') this.state = response.data.state;
        if (response.data.kind === 'environment') this.state = response.data.transition;
      }
      return response;
    };
    const result = this.pending.then(run);
    this.pending = result.then(() => undefined, () => undefined);
    return result;
  }

  private stamp(): Stamp {
    if (!this.state) throw new ProtocolError('Start a game or reset the environment first');
    return { session_id: this.state.session_id, expected_revision: this.state.revision };
  }
  private async expect<K extends ResponseData['kind']>(kind: K, request: Request): Promise<Extract<ResponseData, { kind: K }>> {
    const response = await this.command(request);
    if (response.status === 'error') throw new ApiFault(response);
    if (response.data.kind !== kind) throw new ProtocolError(`Expected ${kind} response`);
    return response.data as Extract<ResponseData, { kind: K }>;
  }
  async newGame(config?: AppConfig): Promise<GameView> {
    const request: Request = config === undefined ? { version: 1, op: 'new' } : { version: 1, op: 'new', config };
    return (await this.expect('game', request)).state;
  }
  async loadConfig(toml: string): Promise<GameView> { return (await this.expect('game', { version: 1, op: 'load_config', toml })).state; }
  async inspectConfig(toml: string): Promise<Extract<ResponseData, { kind: 'config' }>> { return this.expect('config', { version: 1, op: 'inspect_config', toml }); }
  async view(): Promise<GameView> { return (await this.expect('game', { version: 1, op: 'view' })).state; }
  async apply(action: WireAction): Promise<GameView> { return (await this.expect('game', { version: 1, op: 'apply', ...this.stamp(), action })).state; }
  async stepBot(): Promise<GameView> { return (await this.expect('game', { version: 1, op: 'step', ...this.stamp() })).state; }
  async analyze(config: AiConfigSpec = {}): Promise<DecisionView> { return (await this.expect('analysis', { version: 1, op: 'analyze', ...this.stamp(), config })).decision; }
  /** Inspect decompressed records without changing the game; omitted format means mjai. */
  async analyzeMjai(mjson: string, viewer?: SeatId, format?: MjaiFormat): Promise<Extract<ResponseData, { kind: 'mjai' }>> {
    const request: Request = { version: 1, op: 'analyze_mjai', mjson,
      ...(viewer === undefined ? {} : { viewer }), ...(format === undefined ? {} : { format }) };
    return this.expect('mjai', request);
  }
  async environmentReset(seed: Seed = '0', maxSteps = 10_000): Promise<EnvironmentTransition> {
    return (await this.expect('environment', { version: 1, op: 'environment_reset', seed, max_steps: maxSteps })).transition;
  }
  async environmentView(): Promise<EnvironmentTransition> { return (await this.expect('environment', { version: 1, op: 'environment_view' })).transition; }
  async environmentStep(actionId: ActionId): Promise<EnvironmentTransition> {
    return (await this.expect('environment', { version: 1, op: 'environment_step', ...this.stamp(), action_id: actionId })).transition;
  }
  close(): void {
    if (!this.closed) { this.closed = true; this.transport.close?.(); this.state = null; }
  }
}
