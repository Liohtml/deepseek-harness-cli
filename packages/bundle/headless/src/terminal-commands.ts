/** Local terminal setup and discovery; credentials never enter Agent messages. */
import type { Context } from '@deepseek-ai/cordis'
import type { ModelSelection } from '@deepseek-ai/dsh-agent'
import { credentialRef } from '@deepseek-ai/dsh-credentials'
import type {} from '@deepseek-ai/dsh-session-query'
import type { ConversationInput, TerminalEvent } from './terminal.ts'

const API_KEY = credentialRef('DEEPSEEK_API_KEY')
const quote = (value: string): string => `'${value.replaceAll("'", "'\\''")}'`

interface Options {
  cwd: string
  provider?: string
  model?: string
  selectModel?: (selection: ModelSelection) => void
}

/** Session-scoped configuration UI over the existing credential, model and Session services. */
export class TerminalCommands {
  constructor(
    private readonly ctx: Context,
    private readonly input: ConversationInput,
    private readonly write: (text: string) => void,
    private readonly event: ((event: TerminalEvent) => void) | undefined,
    private readonly options: Options,
    private readonly sessionId: string,
  ) {}

  /** Publish credential presence only; no secret is resolved for display. */
  async authStatus(): Promise<void> {
    const credentials = this.ctx.get('credentials')
    if (!credentials) return
    const info = await credentials.describe(API_KEY)
    this.event?.({ type: 'auth', configured: info.configured, source: info.source })
  }

  /**
   * Handle local slash commands without forwarding them to the Agent.
   * @param command - trimmed terminal input.
   * @returns whether this command was handled locally.
   */
  async handle(command: string): Promise<boolean> {
    const name = command.split(/\s/u, 1)[0] ?? ''
    if (!['/login', '/logout', '/status', '/model', '/sessions'].includes(name)) return false
    if (command !== name) {
      this.write(`Use ${name} without arguments. API keys belong only in the masked input.\n`)
      return true
    }
    try {
      if (name === '/login') await this.login()
      else if (name === '/logout') await this.logout()
      else if (name === '/model') await this.model()
      else if (name === '/sessions') await this.sessions()
      else await this.status()
    } catch {
      // Credential/provider failures can contain secret-bearing request details.
      this.write(`${name} could not complete. Check local file permissions and provider configuration, then retry. No credential values are displayed.\n`)
    }
    return true
  }

  private async login(): Promise<void> {
    const credentials = this.ctx.get('credentials')
    if (!credentials || !this.input.askSecret) {
      this.write('Secure key entry requires the current Rust dsh frontend and the local credential service.\n')
      return
    }
    this.write('Connect DeepSeek with an API key\nCreate a key at https://platform.deepseek.com/api_keys\nSaved in the private DSH credential file (not encrypted). Processes running as your OS user can read it.\nThe key is never added to your conversation or input history.\n')
    const answer = await this.input.askSecret('DeepSeek API key · hidden · Enter saves · Esc cancels')
    if (!answer?.trim()) { this.write('Key entry cancelled.\n'); return }
    const key = answer.trim()
    if (key.length > 8192 || /[^\x21-\x7e]/u.test(key)) {
      this.write('Invalid key format. Paste a single API key without whitespace.\n')
      return
    }
    await credentials.set(API_KEY, key)
    await this.authStatus()
    this.write('API key saved. /model chooses a model; /check makes a short API request to verify access.\n')
  }

  private async logout(): Promise<void> {
    const credentials = this.ctx.get('credentials')
    if (!credentials) { this.write('Credential service unavailable.\n'); return }
    const answer = await this.input.ask('Remove the saved DeepSeek API key? [y/N] ')
    if (!/^(y|yes)$/iu.test(answer?.trim() ?? '')) { this.write('Key retained.\n'); return }
    await credentials.unset(API_KEY)
    await this.authStatus()
    const info = await credentials.describe(API_KEY)
    this.write(info.configured
      ? `Saved key removed. A key from ${info.source ?? 'another source'} is still active; remove it there to disconnect.\n`
      : 'Saved key removed. Use /login to connect again.\n')
  }

  private async status(): Promise<void> {
    const info = await this.ctx.get('credentials')?.describe(API_KEY)
    this.write(`Workspace: ${this.options.cwd}\nModel: ${this.options.provider ?? 'unknown'} / ${this.options.model ?? 'unknown'}\nDeepSeek API key: ${info?.configured ? `configured (${info.source ?? 'credential store'})` : 'missing — use /login'}\nSession: ${this.sessionId}\n/check verifies model access with a short API request.\n`)
    await this.authStatus()
  }

  private async model(): Promise<void> {
    const llm = this.ctx.get('llm')
    if (!llm || !this.options.selectModel) { this.write('Model selection unavailable in this profile.\n'); return }
    const models: { provider: string; id: string; name: string }[] = []
    for (const provider of llm.listProviders()) {
      try { models.push(...await llm.listModels(provider.id)) } catch { /* Other registered routes remain selectable. */ }
      if (models.length >= 40) break
    }
    const choices = models.slice(0, 40)
    if (!choices.length) { this.write('No models available. Connect with /login first.\n'); return }
    this.write('Choose a model for this session:\n' + choices.map((item, i) =>
      `${i + 1}. ${item.name} · ${item.provider} / ${item.id}${item.id === this.options.model && item.provider === this.options.provider ? ' · current' : ''}`).join('\n') + '\n')
    const answer = await this.input.ask('Model number (Enter cancels): ')
    if (!answer?.trim()) return
    const number = Number(answer.trim())
    const choice = Number.isInteger(number) && number > 0 ? choices[number - 1] : undefined
    if (!choice) { this.write('Invalid selection. Use /model to choose again.\n'); return }
    this.options.selectModel({ provider: choice.provider, model: choice.id })
    this.options.provider = choice.provider
    this.options.model = choice.id
    this.event?.({ type: 'session', cwd: this.options.cwd, id: this.sessionId, provider: choice.provider, model: choice.id })
    this.write(`Model selected: ${choice.name}. Applies to the next prompt in this session.\n`)
  }

  private async sessions(): Promise<void> {
    const query = this.ctx.get('sessionQuery')
    if (!query) { this.write('Session listing unavailable.\n'); return }
    const sessions = (await query.listSessions()).filter(item => item.persisted
      && item.header.cwd === this.options.cwd && !item.header.parentSession).slice(0, 10)
    if (!sessions.length) { this.write('No saved sessions in this project yet.\n'); return }
    this.write('Recent sessions in this project (exit before resuming another):\n' + sessions.map(({ header }) =>
      `${new Date(header.createdAt).toISOString().slice(0, 16).replace('T', ' ')} UTC${header.id === this.sessionId ? ' · current' : ''}\n  dsh --cwd ${quote(this.options.cwd)} --resume ${quote(header.id)}`).join('\n') + '\n')
  }
}
