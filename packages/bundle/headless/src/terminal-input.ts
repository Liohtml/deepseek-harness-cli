/** Line-oriented terminal input with serialized, cancellable questions and no queued approvals. */
import { createInterface } from 'node:readline/promises'
import { stripVTControlCharacters } from 'node:util'
import type { Readable, Writable } from 'node:stream'

/**
 * Remove terminal escape sequences and control characters from external text.
 * @param text - untrusted text displayed in the terminal.
 * @returns printable text preserving tabs and newlines.
 */
export function terminalText(text: string): string {
  return stripVTControlCharacters(text).replace(/[\x00-\x08\x0b-\x1f\x7f-\x9f]/g, '')
}

/** Owns one readline interface; EOF cancels all current and queued questions. */
export class TerminalInput {
  private readonly lines
  private readonly lifetime = new AbortController()
  private tail: Promise<void> = Promise.resolve()

  /** Create a terminal input owner; onInterrupt decides whether to cancel a turn or exit. */
  constructor(input: Readable, output: Writable, onInterrupt: () => void, onClose: () => void = () => {}) {
    this.lines = createInterface({ input, output, terminal: true })
    this.lines.on('SIGINT', onInterrupt)
    this.lines.on('close', () => { this.lifetime.abort(); onClose() })
  }

  /** Whether EOF, disposal, or an explicit close ended this input. */
  get closed(): boolean { return this.lifetime.signal.aborted }

  /**
   * Ask after earlier questions settle. Cancellation and EOF return undefined.
   * @param prompt - trusted prompt with external values sanitized by the caller.
   * @param signal - optional lifetime of the request being answered.
   * @returns the entered line, or undefined when cancelled.
   */
  async ask(prompt: string, signal?: AbortSignal): Promise<string | undefined> {
    const lifetime = signal ? AbortSignal.any([signal, this.lifetime.signal]) : this.lifetime.signal
    const previous = this.tail
    let release!: () => void
    this.tail = new Promise<void>((resolve) => { release = resolve })
    try {
      await previous
      if (lifetime.aborted) return undefined
      // Text typed while the agent was working must not become an approval answer.
      if (this.lines.line !== '') {
        this.lines.write('', { ctrl: true, name: 'a' })
        this.lines.write('', { ctrl: true, name: 'k' })
      }
      try {
        return await this.lines.question(prompt, { signal: lifetime })
      } catch (error) {
        if (lifetime.aborted) return undefined
        throw error
      }
    } finally {
      release()
    }
  }

  /** Cancel pending reads and restore the terminal. */
  close(): void {
    this.lifetime.abort()
    this.lines.close()
  }
}
