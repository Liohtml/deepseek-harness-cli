/** Loader composition with the real AgentLoop; only model responses and terminal IO are substituted. */
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { PassThrough } from 'node:stream'
import { Context } from '@deepseek-ai/cordis'
import Loader from '@deepseek-ai/cordis-plugin-loader'
import Include from '@deepseek-ai/cordis-plugin-include'
import AgentLoop from '@deepseek-ai/dsh-agent-loop'
import AgentDefaultModel from '@deepseek-ai/dsh-agent-default-model'
import { mountAgentLoopTestDependencies } from '@deepseek-ai/dsh-agent-loop-testkit'
import { provideCmdline } from '@deepseek-ai/dsh-cmdline'
import { installLlmReplay, parseSessionLog } from '@deepseek-ai/dsh-llm-replay'
import { expect, it } from 'vitest'
import { MockAdapter, textResponse } from '../../../core/agent-loop/tests/mock-adapter.ts'
import * as runner from '../src/index.ts'
import * as startup from '../src/startup.ts'
import { internals as startupIo } from '../src/startup-internals.ts'
import { terminalInternals } from '../src/terminal.ts'
import { createTerminalBridge } from '../src/terminal-bridge.ts'

it.each(['followups', 'recorded', 'bridge-recorded'] as const)('boots the interactive Loader composition: %s', async (mode) => {
  const directory = await mkdtemp(join(tmpdir(), 'dsh-terminal-composition-'))
  const ctx = new Context()
  const originalTerminal = { ...terminalInternals }
  const originalStartup = { ...startupIo }
  const model = new MockAdapter([textResponse('First answer.'), textResponse('Second answer.')])
  const exited = Promise.withResolvers<number>()
  let transcript = ''
  const presentation: { type: string; text?: string }[] = []
  const fixture = fileURLToPath(new URL('../../../../snapshots/session/text-turn/session.v1.jsonl', import.meta.url))
  const recorded = mode !== 'followups' ? parseSessionLog(await readFile(fixture, 'utf8')) : []
  const recordedPrompt = recorded.filter(event => event.type === 'user/message')
    .find(event => event.data.source.kind === 'user')?.data.content
    .filter(block => block.type === 'text').map(block => block.text).join('')
  const prompts = mode !== 'followups' ? [recordedPrompt!, '/exit']
    : ['Remember the word terminal.', 'What word did I ask you to remember?', '/exit']
  let closed = false
  terminalInternals.isTTY = () => true
  terminalInternals.write = (text) => { transcript += text }
  terminalInternals.open = () => ({
    get closed() { return closed },
    close: () => { closed = true },
    ask: async () => prompts.shift(),
  })
  const bridgeInput = new PassThrough()
  const bridgeOutput = new PassThrough()
  if (mode === 'bridge-recorded') {
    Object.assign(terminalInternals, createTerminalBridge(bridgeInput, bridgeOutput))
    bridgeOutput.on('data', (chunk) => {
      const frame = JSON.parse(String(chunk)) as { type: string; id?: number; text?: string }
      presentation.push(frame)
      if (frame.type === 'output' || frame.type === 'assistant') transcript += frame.text + '\n'
      if (frame.type === 'close') closed = true
      if (frame.type === 'prompt') queueMicrotask(() => {
        bridgeInput.write(JSON.stringify({ type: 'answer', id: frame.id, text: prompts.shift() }) + '\n')
      })
    })
  }
  startupIo.stdinIsTty = () => true
  try {
    await ctx.plugin(Loader)
    ctx.loader.builtins.include = Include
    ctx.loader.builtins['terminal-test-runtime'] = {
      async apply(context: Context) {
        await mountAgentLoopTestDependencies(context)
        await context.plugin(AgentLoop, { agents: [] })
        await context.plugin(AgentDefaultModel, { provider: 'mock', model: 'mock' })
        if (mode !== 'followups') await context.plugin({
          inject: ['llm'],
          apply(replayCtx: Context) {
            installLlmReplay(replayCtx, { file: fixture, providers: [{ id: 'mock', models: [{ id: 'mock' }] }] })
          },
        })
        else context.get('llm')!.registerAdapter(['mock'], model)
      },
    }
    ctx.loader.builtins['headless-startup'] = startup
    ctx.loader.builtins['headless-runner'] = runner
    provideCmdline(ctx, { args: ['--interactive'], exit: exited.resolve })
    const config = join(directory, 'cordis.yml')
    await writeFile(config, `
- id: runtime
  name: cordis:terminal-test-runtime
- id: startup
  name: cordis:headless-startup
- id: runner
  name: cordis:headless-runner
  inject: [headlessStartup]
  config:
    interactive: !!js ctx.headlessStartup.interactive
    task: !!js ctx.headlessStartup.task
`)
    await ctx.loader.create({ name: 'cordis:include', config: { path: pathToFileURL(config).href } })
    await ctx.loader.await()
    for (const entry of ctx.loader.entries()) await entry.fiber?.await()
    expect(ctx.get('agents')).toBeDefined()
    expect(ctx.get('agentDefaultModel')).toBeDefined()
    expect(ctx.get('headlessStartup')).toBeDefined()
    expect(await exited.promise).toBe(0)
    if (mode === 'followups') {
      expect(model.requests).toHaveLength(2)
      expect(JSON.stringify(model.requests[1])).toContain('Remember the word terminal.')
      expect(JSON.stringify(model.requests[1])).toContain('First answer.')
    }
    expect(closed).toBe(true)
    if (mode === 'bridge-recorded') {
      expect(presentation.some(frame => frame.type === 'session')).toBe(true)
      expect(presentation.some(frame => frame.type === 'assistant_start')).toBe(true)
      expect(presentation.filter(frame => frame.type === 'assistant_delta').map(frame => frame.text).join('')).toBe('PONG')
      expect(presentation.filter(frame => frame.type === 'assistant')).toEqual([{ type: 'assistant', text: 'PONG' }])
    }
    const output = transcript.replace(/Workspace: .*\n/, 'Workspace: <cwd>\n').replace(/Session: .*\n/, 'Session: <session>\n').replace(/\n+$/, '\n')
    await expect(output).toMatchFileSnapshot(fileURLToPath(new URL(`./expected/terminal-${mode}.txt`, import.meta.url)))
  } finally {
    await ctx.fiber.dispose()
    Object.assign(terminalInternals, originalTerminal)
    Object.assign(startupIo, originalStartup)
    bridgeInput.destroy()
    bridgeOutput.destroy()
    await rm(directory, { recursive: true, force: true })
  }
})
