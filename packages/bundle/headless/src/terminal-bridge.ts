/** Version 3 JSON-lines terminal transport; the parent owns the TTY, the profile owns the Agent. */
import { createInterface } from 'node:readline'
import type { Readable, Writable } from 'node:stream'
import type { ConversationInput, ConversationIo } from './terminal.ts'

/**
 * Connect one native frontend to the existing conversation driver.
 * Answers apply only to the active prompt id; cancellation discards stale input.
 * @param input - newline-delimited frontend commands.
 * @param output - newline-delimited display and prompt frames, never terminal bytes.
 * @returns terminal IO whose close cancels pending questions and removes input listeners.
 */
export function createTerminalBridge(input: Readable, output: Writable): ConversationIo {
  const send = (frame: object): void => { output.write(`${JSON.stringify(frame)}\n`) }
  return {
    isTTY: () => true,
    write: (text) => { send({ type: 'output', text }) },
    event: send,
    open(onInterrupt, onClose) {
      const lines = createInterface({ input, terminal: false })
      const lifetime = new AbortController()
      let tail: Promise<void> = Promise.resolve()
      let nextId = 0
      let pending: { id: number; resolve: (text: string | undefined) => void } | undefined
      const close = (): void => {
        if (lifetime.signal.aborted) return
        lifetime.abort()
        lines.close()
        send({ type: 'close' })
        onClose()
      }
      lines.on('close', close)
      lines.on('line', (line) => {
        try {
          const command: unknown = JSON.parse(line)
          if (typeof command !== 'object' || command === null || !('type' in command)) throw new Error('expected a command object')
          switch (command.type) {
            case 'answer':
              if (!('id' in command) || !Number.isSafeInteger(command.id)
                || !('text' in command) || typeof command.text !== 'string') throw new Error('invalid answer')
              if (pending !== undefined && pending.id === command.id) pending.resolve(command.text)
              break
            case 'interrupt': onInterrupt(); break
            case 'close': close(); break
            default: throw new Error('unknown terminal command')
          }
        } catch {
          send({ type: 'error', message: 'Invalid terminal command' })
          close()
        }
      })
      send({ type: 'hello', version: 3 })
      const ask = async (text: string, signal?: AbortSignal, secret = false): Promise<string | undefined> => {
        const previous = tail
        let release!: () => void
        tail = new Promise<void>((resolve) => { release = resolve })
        const abort = signal ? AbortSignal.any([signal, lifetime.signal]) : lifetime.signal
        try {
          await previous
          if (abort.aborted) return undefined
          const id = ++nextId
          return await new Promise<string | undefined>((resolve) => {
            const finish = (answer: string | undefined): void => {
              abort.removeEventListener('abort', cancel)
              pending = undefined
              resolve(answer)
            }
            const cancel = (): void => { send({ type: 'cancel', id }); finish(undefined) }
            pending = { id, resolve: finish }
            abort.addEventListener('abort', cancel, { once: true })
            send(secret ? { type: 'secret_prompt', id, text } : { type: 'prompt', id, text, history: text === 'dsh> ' })
          })
        } finally { release() }
      }
      const reader: ConversationInput = {
        get closed() { return lifetime.signal.aborted },
        close,
        ask,
        askSecret: text => ask(text, undefined, true),
      }
      return reader
    },
  }
}
