/** Interactive driver over the same persisted Agent used by one-shot tasks. */
import type { Context } from '@deepseek-ai/cordis'
import type { Agent } from '@deepseek-ai/dsh-agent'
import type {} from '@deepseek-ai/dsh-user-approval'
import type { AskUserQuestionAnswerItem } from '@deepseek-ai/dsh-user-questions'
import { TerminalInput, terminalText } from './terminal-input.ts'

/** Process-facing terminal creation; tests substitute terminal IO without changing the Agent driver. */
export const terminalInternals: ConversationIo = {
  event: undefined,
  isTTY: () => Boolean(process.stdin.isTTY && process.stdout.isTTY),
  write: (text: string): void => { process.stdout.write(text) },
  open: (onInterrupt: () => void, onClose: () => void): Pick<TerminalInput, 'ask' | 'close' | 'closed'> =>
    new TerminalInput(process.stdin, process.stdout, onInterrupt, onClose),
}

/** Structured presentation facts for the native frontend, separate from durable Session events. */
export type TerminalEvent =
  | { type: 'session'; cwd: string; id: string; provider?: string | undefined; model?: string | undefined }
  | { type: 'assistant_start' }
  | { type: 'assistant_delta' | 'assistant'; text: string }
  | { type: 'tool'; id: string; name: string; arguments: string }
  | { type: 'tool_result'; id: string; failed: boolean; text: string }

/** Terminal IO used by the local readline UI or the native frontend transport. */
export interface ConversationIo {
  /** Whether this endpoint can accept interactive input. */
  isTTY(): boolean
  /** Display a plain informational message. @param text - sanitized display text. */
  write(text: string): void
  /** Optional structured presentation sink; absent uses the plain terminal transcript. */
  event?: ((event: TerminalEvent) => void) | undefined
  /** Open serialized input. @param onInterrupt - cancel or exit callback. @param onClose - input EOF callback. @returns input owner. */
  open(onInterrupt: () => void, onClose: () => void): Pick<TerminalInput, 'ask' | 'close' | 'closed'>
}

const HELP = '/help  commands   /session  resume id   /multiline  enter a block   /exit  quit\nCtrl+C stops a running turn; at the prompt it exits. End a multiline prompt with a single dot.\n'

/**
 * Accept prompts and scoped approval/question requests until EOF or /exit.
 * Unexpected turn failures stop the conversation; completed or cancelled turns may continue.
 * @param ctx - owner of event subscriptions and disposal.
 * @param agent - exclusive root Agent for this terminal conversation.
 * @param options - workspace directory and optional initial prompt.
 * @param run - drive and flush one prompt; nonzero means an unsuccessful turn.
 * @returns when the terminal closes, with no remaining input subscriptions.
 */
export async function runConversation(
  ctx: Context,
  agent: Agent,
  options: {
    cwd: string
    task?: string | undefined
    io?: ConversationIo | undefined
    resumeCommand?: string | undefined
    provider?: string
    model?: string
  },
  run: (prompt: string) => Promise<number>,
): Promise<void> {
  const io = options.io ?? terminalInternals
  if (!io.isTTY()) throw new Error('--interactive requires terminal stdin and stdout')
  const write = (text: string): void => { io.write(terminalText(text)) }
  let running = false
  const input = io.open(() => {
    if (running) {
      agent.cancel({ kind: 'user' })
      write('\nStopping the current turn…\n')
    } else input.close()
  }, () => { if (running) agent.cancel({ kind: 'user' }) })
  const dispose = ctx.effect(() => () => {
    input.close()
    if (running) agent.cancel({ kind: 'user' })
  })
  const stopOutput = ctx.on('session/event', (session, event) => {
    if (session !== agent.session || !running) return
    if (event.type === 'assistant/message') {
      const text = event.data.message.content.filter(block => block.type === 'text').map(block => block.text).join('')
      if (io.event) io.event({ type: 'assistant', text: terminalText(text) })
      else if (text) write(`\n${text}\n\n`)
    } else if (event.type === 'tool/call') {
      if (io.event) io.event({ type: 'tool', id: event.data.callId, name: terminalText(event.data.name), arguments: terminalText(event.data.arguments) })
      else write(`[tool] ${event.data.name}\n${event.data.arguments}\n`)
    } else if (event.type === 'tool/result') {
      if (io.event) io.event({ type: 'tool_result', id: event.data.message.toolCallId,
        failed: event.data.message.isError === true,
        text: terminalText(event.data.message.content.filter(block => block.type === 'text').map(block => block.text).join('\n').slice(0, 8192)),
      })
      else if (event.data.message.isError) write('[tool] failed\n')
    }
  })
  const stopStream = ctx.on('agent/assistant-stream', ({ agent: subject, frame }) => {
    if (subject !== agent || !running || !io.event) return
    if (frame.type === 'start') io.event({ type: 'assistant_start' })
    else if (frame.type === 'chunk' && frame.chunk.type === 'text-delta') {
      io.event({ type: 'assistant_delta', text: terminalText(frame.chunk.text) })
    }
  })
  const stopApproval = ctx.on('approval/request', async (request, next) => {
    if (request.agent !== agent) return next()
    const response = await input.ask(terminalText(`\nAllow ${request.toolName}?\n${request.reason ?? ''}\n[y/N] `), request.signal)
    if (response === undefined) return 'cancelled'
    return /^(y|yes)$/i.test(response.trim()) ? 'allowed-once' : 'rejected'
  })
  const stopQuestions = ctx.on('user-questions/request', async (request, next) => {
    if (request.agent !== agent) return next()
    const answers: AskUserQuestionAnswerItem[] = []
    for (const question of request.questions) {
      const choices = question.options ?? []
      const details = [question.question, question.detail ?? '', ...choices.map((option, index) =>
        `  ${index + 1}. ${option.label}${option.description ? ` — ${option.description}` : ''}`)].filter(Boolean).join('\n')
      const hint = question.multiSelect ? 'Numbers separated by commas, or text: ' : 'Number or text: '
      const response = await input.ask(terminalText(`\n${details}\n${hint}`), request.signal)
      if (response === undefined) return { answers: [] }
      const numbers = response.trim().split(',').map(item => Number(item.trim()))
      const selected = numbers.length > 0 && (question.multiSelect || numbers.length === 1)
        && numbers.every(n => Number.isInteger(n) && n >= 1 && n <= choices.length)
        ? [...new Set(numbers)].flatMap(n => choices.slice(n - 1, n).map(choice => choice.label)) : []
      answers.push({ id: question.id, selected, ...(selected.length ? {} : { custom: response }) })
    }
    return { answers }
  })
  try {
    if (io.event) io.event({ type: 'session', cwd: options.cwd, id: agent.id, provider: options.provider, model: options.model })
    else write(`\nDSH · terminal coding agent\nWorkspace: ${options.cwd}\nSession: ${agent.id}\n${HELP}\n`)
    let prompt = options.task
    while (!input.closed) {
      prompt ??= await input.ask('dsh> ')
      if (prompt === undefined) break
      const command = prompt.trim()
      if (command === '/exit' || command === '/quit') break
      if (command === '/help') write(HELP)
      else if (command === '/session') write(`Session: ${agent.id}\nResume: ${options.resumeCommand ?? 'dsh headless --interactive --session-id'} '${agent.id.replaceAll("'", "'\\''")}'\n`)
      else if (command === '/multiline') {
        const lines: string[] = []
        let line: string | undefined
        while ((line = await input.ask('... ')) !== undefined && line !== '.') lines.push(line)
        if (line === undefined) break
        prompt = lines.join('\n')
        // Multiline content is sent literally, including leading slash commands.
        if (prompt.trim()) {
          running = true
          try {
            if (await run(prompt) !== 0) write('Turn stopped. You can enter a follow-up or /exit.\n')
          } finally { running = false }
        }
      } else if (command.startsWith('/')) write('Unknown command. Type /help; use /multiline to send literal slash-prefixed text.\n')
      else if (command) {
        running = true
        try {
          if (await run(prompt) !== 0) write('Turn stopped. You can enter a follow-up or /exit.\n')
        } finally { running = false }
      }
      prompt = undefined
    }
  } finally {
    stopQuestions()
    stopStream()
    stopApproval()
    stopOutput()
    dispose()
  }
}
