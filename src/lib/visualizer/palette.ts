import { artworkApi } from '@/api'
import { fallbackPalette, type Palette } from './renderer'

export function themePalette(): Palette {
  try {
    const style = getComputedStyle(document.documentElement)
    const canvas = document.createElement('canvas')
    canvas.width = 1
    canvas.height = 1
    const context = canvas.getContext('2d')
    if (!context) return fallbackPalette
    return ['--artwork-a', '--artwork-b'].map((name) => {
      const color = style.getPropertyValue(name).trim()
      if (!color) throw new Error('Theme colors are unavailable')
      context.fillStyle = color
      context.fillRect(0, 0, 1, 1)
      return [...context.getImageData(0, 0, 1, 1).data].slice(0, 3).map((value) => value / 255)
    }) as Palette
  } catch {
    return fallbackPalette
  }
}

/** Read a small image only once per track. Cross-origin failures use the theme palette. */
export async function artworkPalette(videoId: string): Promise<Palette> {
  const fallback = themePalette()
  const source = await artworkApi.resolveYouTube(videoId)
  if (!source) return fallback
  return new Promise((resolve) => {
    const image = new Image()
    let settled = false
    const finish = (palette: Palette) => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      image.onload = null
      image.onerror = null
      resolve(palette)
    }
    const timer = setTimeout(() => finish(fallback), 3000)
    image.crossOrigin = 'anonymous'
    image.onload = () => {
      try {
        const canvas = document.createElement('canvas')
        canvas.width = 8
        canvas.height = 8
        const context = canvas.getContext('2d')
        if (!context) {
          finish(fallback)
          return
        }
        context.drawImage(image, 0, 0, 8, 8)
        const pixels = context.getImageData(0, 0, 8, 8).data
        const colors = [
          [0, 0, 0],
          [0, 0, 0],
        ]
        const counts = [0, 0]
        for (let i = 0; i < pixels.length; i += 4) {
          const side = i % 32 < 16 ? 0 : 1
          for (let c = 0; c < 3; c++) colors[side]![c]! += pixels[i + c]! / 255
          counts[side]!++
        }
        finish(
          colors.map((color, side) =>
            color.map((value) => Math.min(0.95, Math.max(0.18, value / counts[side]!))),
          ) as Palette,
        )
      } catch {
        finish(fallback)
      }
    }
    image.onerror = () => finish(fallback)
    image.src = source
  })
}
