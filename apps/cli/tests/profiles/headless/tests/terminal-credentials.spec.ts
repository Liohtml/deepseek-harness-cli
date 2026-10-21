/** Credential activation through the shipped headless patch and real Loader. */
import { fileURLToPath } from 'node:url'
import { Context } from '@deepseek-ai/cordis'
import Loader from '@deepseek-ai/cordis-plugin-loader'
import { composeEntries, loadOverlayPatches } from '@deepseek-ai/dsh-app-boot'
import { provideCmdline } from '@deepseek-ai/dsh-cmdline'
import { expect, it } from 'vitest'

const layers = ['base', 'headless'].map(name => loadOverlayPatches('credentials test',
  fileURLToPath(new URL(`../../../../../../packages/bundle/${name}/cordis.patch.yml`, import.meta.url))))

it.each([
  { args: ['--terminal-bridge'], enabled: true },
  { args: ['--interactive'], enabled: true },
  { args: ['-i'], enabled: true },
  { args: ['--session-id', 'session-example', '--terminal-bridge'], enabled: true },
  { args: ['Explain this project'], enabled: false },
  { args: ['--json', 'Explain this project'], enabled: false },
  { args: [], enabled: false },
  { args: ['--', '--interactive'], enabled: false },
  { args: ['--', '--terminal-bridge'], enabled: false },
  { args: ['--', '-i'], enabled: false },
])('managed credentials for $args: $enabled', async ({ args, enabled }) => {
  const ctx = new Context()
  try {
    await ctx.plugin(Loader)
    provideCmdline(ctx, { args, exit: () => {} })
    let mounted = false
    ctx.loader.builtins['test-credentials'] = { apply: () => { mounted = true } }
    const row = composeEntries(layers).find(entry => entry.id === 'credentials')!
    await ctx.loader.create({ ...row, name: 'cordis:test-credentials' })
    await ctx.loader.await()
    expect(mounted).toBe(enabled)
  } finally {
    await ctx.fiber.dispose()
  }
})

it('allows an explicit overlay to restore the store for one-shot jobs', async () => {
  const row = composeEntries([...layers, [{ id: 'credentials', disabled: false }]])
    .find(entry => entry.id === 'credentials')!
  expect(row.disabled).toBe(false)
})
