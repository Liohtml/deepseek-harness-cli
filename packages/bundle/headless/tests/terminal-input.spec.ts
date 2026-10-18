/** Real readline cancellation, EOF, serialization, and terminal output safety. */
import { PassThrough } from 'node:stream'
import { afterEach, expect, it } from 'vitest'
import { TerminalInput, terminalText } from '../src/terminal-input.ts'

const cleanup: (() => void)[] = []
afterEach(() => { cleanup.splice(0).forEach(dispose => dispose()) })

function terminal() {
  const stdin = new PassThrough()
  const stdout = new PassThrough()
  const input = new TerminalInput(stdin, stdout, () => input.close())
  cleanup.push(() => { input.close(); stdin.destroy(); stdout.destroy() })
  return { input, stdin, stdout }
}

it('does not reuse text typed before an approval is asked', async () => {
  const { input, stdin } = terminal()
  stdin.write('yes\n')
  stdin.write('yes')
  const answer = input.ask('Allow? ')
  await Promise.resolve()
  stdin.write('no\n')
  await expect(answer).resolves.toBe('no')
})

it('serializes concurrent questions', async () => {
  const { input, stdin } = terminal()
  const first = input.ask('First? ')
  const second = input.ask('Second? ')
  await Promise.resolve()
  stdin.write('one\n')
  await expect(first).resolves.toBe('one')
  stdin.write('two\n')
  await expect(second).resolves.toBe('two')
})

it('cancels a question without consuming the next answer', async () => {
  const { input, stdin } = terminal()
  const abort = new AbortController()
  const question = input.ask('Cancelled? ', abort.signal)
  await Promise.resolve()
  abort.abort()
  await expect(question).resolves.toBeUndefined()
  const next = input.ask('Next? ')
  await Promise.resolve()
  stdin.write('answer\n')
  await expect(next).resolves.toBe('answer')
})

it('settles pending and queued questions on EOF', async () => {
  const { input, stdin } = terminal()
  const first = input.ask('First? ')
  const second = input.ask('Second? ')
  await Promise.resolve()
  stdin.end()
  await expect(first).resolves.toBeUndefined()
  await expect(second).resolves.toBeUndefined()
  expect(input.closed).toBe(true)
})

it('strips terminal controls while preserving ordinary Unicode and layout', () => {
  expect(terminalText('\x1b[2Jhello\x1b]52;c;secret\x07\r\x08\x00\n\tGrüße')).toBe('hello\n\tGrüße')
})
