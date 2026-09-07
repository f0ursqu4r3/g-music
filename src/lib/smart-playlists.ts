import type {
  Playlist,
  SmartPlaylistDefinition,
  SmartPlaylistField,
  SmartPlaylistOperator,
  SmartPlaylistRule,
} from '@/api'

export const ruleFields: Record<SmartPlaylistField, string> = {
  title: 'Title',
  artist: 'Artist',
  album: 'Album',
  label: 'Label',
  genre: 'Genre',
  durationMs: 'Duration (milliseconds)',
  playCount: 'Play count',
  favorite: 'Favorite',
  lastPlayedDays: 'Last played (days)',
}
export const operatorLabels: Record<SmartPlaylistOperator, string> = {
  contains: 'contains',
  equals: 'equals',
  notContains: 'does not contain',
  lessThan: 'is less than',
  greaterThan: 'is greater than',
  within: 'within',
  notWithin: 'not within',
}
export function operatorsFor(field: SmartPlaylistField): SmartPlaylistOperator[] {
  if (field === 'favorite') return ['equals']
  if (field === 'lastPlayedDays') return ['within', 'notWithin']
  if (field === 'durationMs' || field === 'playCount') return ['equals', 'lessThan', 'greaterThan']
  return ['contains', 'equals', 'notContains']
}
export function newRule(field: SmartPlaylistField = 'title'): SmartPlaylistRule {
  return {
    field,
    operator: operatorsFor(field)[0]!,
    value:
      field === 'favorite'
        ? true
        : field === 'lastPlayedDays'
          ? 30
          : field === 'durationMs' || field === 'playCount'
            ? 0
            : '',
  }
}
export function ordinaryPlaylist(playlist: Playlist): boolean {
  return !playlist.smart && playlist.id !== 'favorites' && playlist.id !== 'most-played'
}
export function describeRule(rule: SmartPlaylistRule): string {
  if (rule.field === 'favorite') return rule.value ? 'Is a favorite' : 'Is not a favorite'
  if (rule.field === 'lastPlayedDays')
    return `Last played ${operatorLabels[rule.operator]} ${rule.value} days${rule.operator === 'notWithin' ? ' (includes never played)' : ''}`
  return `${ruleFields[rule.field]} ${operatorLabels[rule.operator]} ${rule.value}`
}
export function validateDefinition(definition: SmartPlaylistDefinition): string {
  if (!['all', 'any'].includes(definition.match)) return 'Choose All or Any.'
  if (definition.rules.length < 1 || definition.rules.length > 20) return 'Use 1 to 20 rules.'
  for (const [index, rule] of definition.rules.entries()) {
    const prefix = `Rule ${index + 1}: `
    if (!(rule.field in ruleFields) || !operatorsFor(rule.field).includes(rule.operator))
      return prefix + 'Choose a supported field and operator.'
    if (rule.field === 'favorite') {
      if (typeof rule.value !== 'boolean') return prefix + 'Choose Yes or No.'
    } else if (['durationMs', 'playCount', 'lastPlayedDays'].includes(rule.field)) {
      if (typeof rule.value !== 'number' || !Number.isSafeInteger(rule.value) || rule.value < 0)
        return prefix + 'Enter a non-negative safe integer.'
      if (rule.field === 'lastPlayedDays' && (rule.value < 1 || rule.value > 36500))
        return prefix + 'Enter 1 to 36500 days.'
    } else if (
      typeof rule.value !== 'string' ||
      !rule.value.trim() ||
      Array.from(rule.value.trim()).length > 200
    )
      return prefix + 'Enter 1 to 200 characters.'
  }
  if (
    ![
      'libraryOrder',
      'title',
      'artist',
      'album',
      'durationMs',
      'playCount',
      'lastPlayedAtMs',
    ].includes(definition.sort.field) ||
    !['asc', 'desc'].includes(definition.sort.direction)
  )
    return 'Choose a supported sort order.'
  if (
    definition.limit !== null &&
    (!Number.isInteger(definition.limit) || definition.limit < 1 || definition.limit > 10000)
  )
    return 'Enter a limit from 1 to 10000, or leave it empty.'
  return ''
}
export const smartPresets: { id: string; name: string; definition: SmartPlaylistDefinition }[] = [
  {
    id: 'never-played',
    name: 'Never played',
    definition: {
      match: 'all',
      rules: [{ field: 'playCount', operator: 'equals', value: 0 }],
      sort: { field: 'libraryOrder', direction: 'asc' },
      limit: null,
    },
  },
  {
    id: 'forgotten-favorites',
    name: 'Forgotten favorites',
    definition: {
      match: 'all',
      rules: [
        { field: 'favorite', operator: 'equals', value: true },
        { field: 'lastPlayedDays', operator: 'notWithin', value: 30 },
      ],
      sort: { field: 'lastPlayedAtMs', direction: 'asc' },
      limit: null,
    },
  },
  {
    id: 'short-tracks',
    name: 'Short tracks',
    definition: {
      match: 'all',
      rules: [{ field: 'durationMs', operator: 'lessThan', value: 180000 }],
      sort: { field: 'durationMs', direction: 'asc' },
      limit: null,
    },
  },
]
export function copyDefinition(definition: SmartPlaylistDefinition): SmartPlaylistDefinition {
  return {
    ...definition,
    rules: definition.rules.map((rule) => ({ ...rule })),
    sort: { ...definition.sort },
  }
}
