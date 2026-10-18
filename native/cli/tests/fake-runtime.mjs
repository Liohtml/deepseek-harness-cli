// Native frontend acceptance fixture: no model or filesystem tools are run.
import { createInterface } from 'node:readline'
import { writeFileSync } from 'node:fs'

const send = frame => process.stdout.write(JSON.stringify(frame) + '\n')
writeFileSync(process.env.DSH_TEST_RECORD, JSON.stringify({ args: process.argv.slice(2), cwd: process.cwd() }))
send({ type: 'hello', version: 2 })
send({ type: 'session', cwd: process.cwd(), id: 'session-test', model: 'test-model', provider: 'fixture' })
send({ type: 'prompt', id: 1, text: 'dsh> ', history: true })
let id = 1
let waiting = false
createInterface({ input: process.stdin }).on('line', line => {
  const command = JSON.parse(line)
  if (command.type === 'close' || command.text === '/exit') {
    send({ type: 'close' })
    process.exit(0)
  }
  if (command.type === 'interrupt') {
    send({ type: 'cancel', id })
    send({ type: 'output', text: 'Turn cancelled\n' })
    send({ type: 'prompt', id: ++id, text: 'dsh> ', history: true })
    return
  }
  if (command.type !== 'answer' || command.id !== id) throw new Error('Wrong answer identity')
  if (waiting) {
    waiting = false
    send({ type: 'output', text: `Approval answer: ${command.text}\n` })
    send({ type: 'prompt', id: ++id, text: 'dsh> ', history: true })
  } else if (command.text === 'permission') {
    send({ type: 'output', text: 'Working before permission\n' })
    waiting = true
    setTimeout(() => send({ type: 'prompt', id: ++id, text: 'Allow tool?\n[y/N] ', history: false }), 150)
  } else if (command.text === 'wait') {
    send({ type: 'output', text: 'Working until interrupt\n' })
  } else {
    send({ type: 'assistant_start' })
    send({ type: 'assistant_delta', text: 'Answer: ' })
    send({ type: 'assistant_delta', text: command.text })
    send({ type: 'assistant', text: `Answer: ${command.text}` })
    send({ type: 'prompt', id: ++id, text: 'dsh> ', history: true })
  }
})
