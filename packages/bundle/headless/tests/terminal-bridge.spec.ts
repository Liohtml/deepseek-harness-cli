import { PassThrough } from 'node:stream'
import { expect, it, vi } from 'vitest'
import { createTerminalBridge } from '../src/terminal-bridge.ts'

function setup() {
  const input = new PassThrough()
  const output = new PassThrough()
  const frames: Record<string, unknown>[] = []
  output.on('data', (chunk) => { frames.push(JSON.parse(String(chunk))) })
  const interrupt = vi.fn()
  const closed = vi.fn()
  const io = createTerminalBridge(input, output)
  const terminal = io.open(interrupt, closed)
  const send = (command: object) => { input.write(JSON.stringify(command) + '\n') }
  return { input, output, frames, terminal, io, interrupt, closed, send }
}

it('serializes questions and discards typeahead and duplicate answers', async () => {
  const { terminal, frames, send, io } = setup()
  expect(frames.shift()).toEqual({ type: 'hello', version: 2 })
  io.write('Hello\nworld\n')
  expect(frames.shift()).toEqual({ type: 'output', text: 'Hello\nworld\n' })
  send({ type: 'answer', id: 1, text: 'stale' })
  const first = terminal.ask('dsh> ')
  const second = terminal.ask('[y/N] ')
  await vi.waitFor(() => { expect(frames).toHaveLength(1) })
  expect(frames.shift()).toEqual({ type: 'prompt', id: 1, text: 'dsh> ', history: true })
  send({ type: 'answer', id: 1, text: 'work' })
  send({ type: 'answer', id: 1, text: 'yes' })
  expect(await first).toBe('work')
  await vi.waitFor(() => { expect(frames).toHaveLength(1) })
  expect(frames.shift()).toEqual({ type: 'prompt', id: 2, text: '[y/N] ', history: false })
  send({ type: 'answer', id: 2, text: 'no' })
  expect(await second).toBe('no')
  terminal.close()
})

it('cancels active requests and accepts only the next prompt identity', async () => {
  const { terminal, frames, send } = setup()
  const controller = new AbortController()
  const first = terminal.ask('Permission?', controller.signal)
  await vi.waitFor(() => { expect(frames.at(-1)?.type).toBe('prompt') })
  controller.abort()
  expect(await first).toBeUndefined()
  expect(frames.at(-1)).toEqual({ type: 'cancel', id: 1 })
  const next = terminal.ask('dsh> ')
  await vi.waitFor(() => { expect(frames.at(-1)?.id).toBe(2) })
  send({ type: 'answer', id: 1, text: 'yes' })
  send({ type: 'answer', id: 2, text: 'new prompt' })
  expect(await next).toBe('new prompt')
  terminal.close()
})

it.each(['eof', 'close', 'malformed'] as const)('settles active and queued reads on %s', async (mode) => {
  const { input, terminal, frames, send, closed, interrupt } = setup()
  const first = terminal.ask('first')
  const second = terminal.ask('second')
  await vi.waitFor(() => { expect(frames.at(-1)?.type).toBe('prompt') })
  send({ type: 'interrupt' })
  expect(interrupt).toHaveBeenCalledOnce()
  if (mode === 'eof') input.end()
  else if (mode === 'close') send({ type: 'close' })
  else input.write('{invalid}\n')
  expect(await first).toBeUndefined()
  expect(await second).toBeUndefined()
  expect(terminal.closed).toBe(true)
  expect(closed).toHaveBeenCalledOnce()
  expect(frames.at(-1)).toEqual({ type: 'close' })
  if (mode === 'malformed') expect(frames.some(frame => frame.type === 'error')).toBe(true)
})
