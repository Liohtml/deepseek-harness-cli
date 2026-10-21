import { afterEach, expect, it, vi } from 'vitest'
import { Context } from '@deepseek-ai/cordis'
import { TerminalCommands } from '../src/terminal-commands.ts'
import type { ConversationInput, TerminalEvent } from '../src/terminal.ts'

let ctx: Context
const secret = 'sk-fixture-not-a-real-api-key'
afterEach(async () => { await ctx?.fiber.dispose() })

function fixture(answers: string[] = []) {
  ctx = new Context()
  let stored: string | undefined
  const credentials = {
    describe: vi.fn(async () => ({ configured: stored !== undefined, source: stored ? 'file' : undefined, writable: true })),
    set: vi.fn(async (_ref: string, value: string) => { stored = value }),
    unset: vi.fn(async () => { stored = undefined }),
  }
  ctx.provide('credentials', credentials as never)
  const output: string[] = []
  const events: TerminalEvent[] = []
  const input: ConversationInput = {
    closed: false, close: () => {},
    ask: vi.fn(async () => answers.shift()),
    askSecret: vi.fn(async () => secret),
  }
  const options = { cwd: '/project', provider: 'deepseek-official', model: 'first', selectModel: vi.fn() }
  const commands = new TerminalCommands(ctx, input, text => output.push(text), event => events.push(event), options, 'session-fixture')
  return { commands, credentials, input, output, events, options }
}

it('uses masked input and publishes presence without exposing the key', async () => {
  const f = fixture()
  expect(await f.commands.handle('/login')).toBe(true)
  expect(f.input.askSecret).toHaveBeenCalledOnce()
  expect(f.input.ask).not.toHaveBeenCalled()
  expect(f.credentials.set).toHaveBeenCalledWith('DEEPSEEK_API_KEY', secret)
  expect(f.events).toContainEqual({ type: 'auth', configured: true, source: 'file' })
  expect(JSON.stringify([f.output, f.events])).not.toContain(secret)
  expect(f.output.join('')).toContain('not encrypted')
})

it('refuses inline keys and unmasked frontends without changing credentials', async () => {
  const f = fixture()
  await f.commands.handle(`/login ${secret}`)
  delete f.input.askSecret
  await f.commands.handle('/login')
  expect(f.credentials.set).not.toHaveBeenCalled()
  expect(f.input.ask).not.toHaveBeenCalled()
  expect(f.output.join('')).not.toContain(secret)
})

it('cancels empty input and contains secret-bearing storage errors', async () => {
  const f = fixture()
  f.input.askSecret = async () => ''
  await f.commands.handle('/login')
  expect(f.credentials.set).not.toHaveBeenCalled()
  f.input.askSecret = async () => secret
  f.credentials.set.mockRejectedValueOnce(new Error(`write failed: ${secret}`))
  await f.commands.handle('/login')
  expect(f.output.join('')).toContain('could not complete')
  expect(JSON.stringify([f.output, f.events])).not.toContain(secret)
})

it('confirms removal and reports an ambient credential that remains active', async () => {
  const f = fixture(['no', 'yes'])
  await f.commands.handle('/logout')
  expect(f.credentials.unset).not.toHaveBeenCalled()
  f.credentials.describe.mockResolvedValue({ configured: true, source: 'env', writable: true })
  await f.commands.handle('/logout')
  expect(f.credentials.unset).toHaveBeenCalledOnce()
  expect(f.output.join('')).toContain('env is still active')
})

it('selects a catalog model without sending an Agent prompt', async () => {
  const f = fixture(['2'])
  ctx.provide('llm', {
    listProviders: () => [{ id: 'deepseek-official', name: 'DeepSeek' }],
    listModels: async () => ['first', 'second'].map(id => ({ provider: 'deepseek-official', id, name: id })),
  } as never)
  await f.commands.handle('/model')
  expect(f.options.selectModel).toHaveBeenCalledWith({ provider: 'deepseek-official', model: 'second' })
  expect(f.events.at(-1)).toMatchObject({ type: 'session', model: 'second' })
  await f.commands.handle('/status')
  expect(f.output.join('')).toContain('deepseek-official / second')
  expect(await f.commands.handle('ordinary coding task')).toBe(false)
})
