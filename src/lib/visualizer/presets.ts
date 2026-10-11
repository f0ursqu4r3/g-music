export const visualizerPresets = [
  {
    id: 'spectrum',
    name: 'Spectrum',
    family: 'Essential',
    description: 'Sixteen luminous towers. Every band in view.',
    accent: '#56e8d7',
  },
  {
    id: 'flow',
    name: 'Flow',
    family: 'Atmosphere',
    description: 'Soft ribbons carried by the midrange.',
    accent: '#f197b2',
  },
  {
    id: 'aurora',
    name: 'Aurora',
    family: 'Atmosphere',
    description: 'Polar curtains with electric green edges.',
    accent: '#72ffc3',
  },
  {
    id: 'plasma',
    name: 'Plasma',
    family: 'Atmosphere',
    description: 'Molten color fields that fold into the bass.',
    accent: '#ff69cd',
  },
  {
    id: 'solar',
    name: 'Solar flare',
    family: 'Cosmos',
    description: 'A burning corona. Bass fuels the fire.',
    accent: '#ffac52',
  },
  {
    id: 'eclipse',
    name: 'Eclipse',
    family: 'Cosmos',
    description: 'A dark core inside a luminous accretion disk.',
    accent: '#ffcda3',
  },
  {
    id: 'tunnel',
    name: 'Hyperdrive',
    family: 'Electric',
    description: 'Neon portals through a twisting tunnel.',
    accent: '#b48aff',
  },
  {
    id: 'starfield',
    name: 'Starfield',
    family: 'Cosmos',
    description: 'Layers of stars with treble-lit trails.',
    accent: '#91d8ff',
  },
  {
    id: 'nova',
    name: 'Supernova',
    family: 'Cosmos',
    description: 'Stellar rays expand with the low end.',
    accent: '#ff768b',
  },
  {
    id: 'prism',
    name: 'Prism',
    family: 'Geometry',
    description: 'Nested glass triangles split the spectrum.',
    accent: '#93a6ff',
  },
  {
    id: 'kaleidoscope',
    name: 'Kaleidoscope',
    family: 'Geometry',
    description: 'Mirrored petals in a shifting color mosaic.',
    accent: '#ff87e2',
  },
  {
    id: 'mandala',
    name: 'Mandala',
    family: 'Geometry',
    description: 'Fine radial lace traced by each frequency.',
    accent: '#ffd18b',
  },
  {
    id: 'grid',
    name: 'Neon highway',
    family: 'Electric',
    description: 'A violet horizon over a racing laser grid.',
    accent: '#f878f1',
  },
  {
    id: 'helix',
    name: 'Double helix',
    family: 'Geometry',
    description: 'Two luminous strands woven around the beat.',
    accent: '#8cf1ec',
  },
  {
    id: 'rings',
    name: 'Orbit',
    family: 'Geometry',
    description: 'Tilted frequency rings around a bright core.',
    accent: '#ffd36e',
  },
  {
    id: 'silk',
    name: 'Silk',
    family: 'Atmosphere',
    description: 'Interlaced threads of iridescent light.',
    accent: '#b9a0ff',
  },
  {
    id: 'vortex',
    name: 'Vortex',
    family: 'Electric',
    description: 'Spiral arms pull color into a deep center.',
    accent: '#66e4ff',
  },
  {
    id: 'sparks',
    name: 'Fireflies',
    family: 'Atmosphere',
    description: 'Drifting points of light in a midnight garden.',
    accent: '#d3ff8a',
  },
  {
    id: 'terrain',
    name: 'Signal peaks',
    family: 'Electric',
    description: 'Layered mountain ridges built from the bands.',
    accent: '#fd8aab',
  },
  {
    id: 'chrome',
    name: 'Liquid chrome',
    family: 'Atmosphere',
    description: 'Polished ripples with cyan and rose reflections.',
    accent: '#aedcff',
  },
] as const

export type VisualizerPresetId = (typeof visualizerPresets)[number]['id']
export type VisualizerPreset = (typeof visualizerPresets)[number]
export function isVisualizerPreset(value: unknown): value is VisualizerPresetId {
  return visualizerPresets.some((preset) => preset.id === value)
}
export function adjacentPreset(id: VisualizerPresetId, direction: number): VisualizerPresetId {
  const index = visualizerPresets.findIndex((preset) => preset.id === id)
  return visualizerPresets[
    (index + direction + visualizerPresets.length) % visualizerPresets.length
  ]!.id
}
