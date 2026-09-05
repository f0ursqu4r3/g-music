import { copyFileSync, mkdtempSync, readdirSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('../', import.meta.url))
const output = mkdtempSync(join(tmpdir(), 'gmusic-icons-'))

try {
  const result = spawnSync(
    process.execPath,
    ['run', 'tauri', 'icon', 'public/app-icon.svg', '--output', output],
    { cwd: root, stdio: 'inherit' },
  )
  if (result.error) throw result.error
  if (result.status !== 0) throw new Error('Tauri icon generation failed')

  // Tauri also generates mobile assets. Keep only desktop formats in this app.
  for (const file of readdirSync(output, { withFileTypes: true })) {
    if (!file.isFile() || !/\.(png|icns|ico)$/.test(file.name)) continue
    copyFileSync(join(output, file.name), join(root, 'src-tauri', 'icons', file.name))
  }
} finally {
  rmSync(output, { recursive: true, force: true })
}
