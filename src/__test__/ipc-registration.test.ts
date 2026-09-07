import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const api = readFileSync('src/api.ts', 'utf8')
const backend = readFileSync('src-tauri/src/commands.rs', 'utf8')
const windows = readFileSync('src-tauri/src/windows.rs', 'utf8')
const bootstrap = readFileSync('src-tauri/src/lib.rs', 'utf8')
const handler = bootstrap.split('tauri::generate_handler![')[1]?.split(']')[0]
const commands = [
  ...new Set([...api.matchAll(/invoke(?:<[^;]*?>)?\(\s*['"](\w+)['"]/g)].map((match) => match[1])),
]

describe('frontend to native command registration', () => {
  it('includes the startup import inspection in the checked contract', () => {
    expect(commands).toContain('inspect_import_progress')
    expect(handler).toBeDefined()
  })

  it.each(commands)('implements and registers %s', (command) => {
    const module = command === 'show_app_window' ? 'windows' : 'commands'
    expect(module === 'windows' ? windows : backend).toMatch(
      new RegExp(`pub (?:async )?fn ${command}\\b`),
    )
    expect(handler).toMatch(new RegExp(`${module}::${command}\\b`))
  })
})
