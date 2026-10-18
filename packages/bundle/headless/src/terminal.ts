/** Interactive driver over the same persisted Agent used by one-shot tasks. */
import type { Context } from '@deepseek-ai/cordis'
import type { Agent } from '@deepseek-ai/dsh-agent'
import type {} from '@deepseek-ai/dsh-user-approval'
import type { AskUserQuestionAnswerItem } from '@deepseek-ai/dsh-user-questions'
import { TerminalInput, terminalText } from './terminal-input.ts'

/** Process-facing terminal creation; tests substitute terminal IO without changing the Agent driver. */
export const terminalInternals = {
  isTTY: () => Boolean(process.stdin.isTTY && process.stdout.isTTY),
  write: (text: string): void => { process.stdout.write(text) },
  open: (onInterrupt: () => void, onClose: () => void): Pick<TerminalInput, 'ask' | 'close' | 'closed'> =>
    new TerminalInput(process.stdin, process.stdout, onInterrupt, onClose),
}

/** Terminal IO used by the local readline UI or the native frontend transport. */
export type ConversationIo = typeof terminalInternals

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
  options: { cwd: string; task?: string | undefined; io?: ConversationIo | undefined; resumeCommand?: string | undefined },
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
      if (text) write(`\n${text}\n\n`)
    } else if (event.type === 'tool/call') {
      write(`[tool] ${event.data.name}\n${event.data.arguments}\n`)
    } else if (event.type === 'tool/result' && event.data.message.isError) {
      write('[tool] failed\n')
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
        ? [...new Set(numbers)].map(n => choices[n - 1]!.label) : []
      answers.push({ id: question.id, selected, ...(selected.length ? {} : { custom: response }) })
    }
    return { answers }
  })
  try {
    write(`\nDSH · terminal coding agent\nWorkspace: ${options.cwd}\nSession: ${agent.id}\n${HELP}\n`)
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
    stopApproval()
    stopOutput()
    dispose()
  }
}
