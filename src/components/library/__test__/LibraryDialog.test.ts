import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it } from 'vitest'

import LibraryDialog from '../LibraryDialog.vue'

const menuSurfaceClasses = [
  'rounded-lg',
  'border',
  'border-(--line)',
  'bg-(--menu-surface)',
  'text-(--text)',
  'shadow-xl',
]

let wrapper: ReturnType<typeof mount> | undefined

afterEach(() => {
  wrapper?.unmount()
  wrapper = undefined
})

describe('modal surface consistency', () => {
  it('renders library dialogs with the context menu surface treatment', async () => {
    wrapper = mount(LibraryDialog, {
      attachTo: document.body,
      props: { title: 'Edit library' },
      slots: { default: '<button>Save changes</button>' },
    })
    await flushPromises()
    const dialog = document.querySelector('[role="dialog"]')!
    expect(dialog).not.toBeNull()
    for (const name of menuSurfaceClasses) expect(dialog.classList).toContain(name)
    expect(dialog.getAttribute('aria-modal')).toBe('true')
    expect(dialog.textContent).toContain('Save changes')
  })

  it('keeps every modal shell aligned with the canonical context menu', () => {
    const menu = readFileSync(
      join(process.cwd(), 'src/components/ui/context-menu/ContextMenuContent.vue'),
      'utf8',
    )
    for (const name of menuSurfaceClasses) expect(menu).toContain(name)
    for (const file of [
      'src/components/library/LibraryDialog.vue',
      'src/components/QueueWindow.vue',
      'src/App.vue',
    ]) {
      const source = readFileSync(join(process.cwd(), file), 'utf8')
      const shells = [...source.matchAll(/<DialogContent\b[\s\S]*?class="([^"]+)"/g)]
      expect(shells.length, file).toBeGreaterThan(0)
      for (const shell of shells) {
        for (const name of menuSurfaceClasses) {
          expect(shell[1]!.split(/\s+/), `${file}: ${name}`).toContain(name)
        }
      }
    }
  })

  it('uses the menu surface behind sticky modal actions in every theme', () => {
    const source = readFileSync(join(process.cwd(), 'src/styles.css'), 'utf8')
    expect(source).toContain('--dialog-surface: var(--menu-surface)')
  })
})
