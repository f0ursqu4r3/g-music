import type { VisualizerSettings } from './settings'
import type { VisualizerSignal } from './dynamics'
import { feedbackSource, feedbackStyles, fragmentSource, presentSource } from './shaders'
import type { VisualizerPresetId } from './presets'
import { renderSize } from './render-size'
import type { VisualizerVideo } from './video'

export type Palette = [number[], number[]]
export const fallbackPalette: Palette = [
  [0.42, 0.73, 0.85],
  [0.84, 0.57, 0.39],
]
const vertex = `#version 300 es
void main(){vec2 p=vec2(float((gl_VertexID<<1)&2),float(gl_VertexID&2));gl_Position=vec4(p*2.-1.,0.,1.);}`
type Uniforms = Record<string, WebGLUniformLocation | null>
interface Program {
  program: WebGLProgram
  uniforms: Uniforms
}
interface Target {
  texture: WebGLTexture
  framebuffer: WebGLFramebuffer
}
const sceneUniforms = [
  'resolution',
  'visualBounds',
  'bands[0]',
  'peaks[0]',
  'time',
  'intensity',
  'levels',
  'hits',
  'swell',
  'colorA',
  'colorB',
]
export class VisualizerRenderer {
  private gl: WebGL2RenderingContext
  private programs = new Map<VisualizerPresetId, Program>()
  private feedback: Program
  private present: Program
  private scene: Target | null = null
  private history: Target[] = []
  private size = { width: 0, height: 0 }
  private videoTexture: WebGLTexture | null = null
  private videoFilled = false
  /** Set when the webview refuses to share video pixels with WebGL. */
  videoBlocked = false
  constructor(
    private canvas: HTMLCanvasElement,
    private preview = false,
  ) {
    const gl = canvas.getContext('webgl2', {
      alpha: false,
      antialias: false,
      depth: false,
      powerPreference: 'low-power',
    })
    if (!gl)
      throw new Error('WebGL2 is unavailable. Update your graphics driver or reopen this window.')
    this.gl = gl
    this.feedback = this.compile(feedbackSource, [
      'scene',
      'history',
      'resolution',
      'center',
      'drift',
      'decay',
      'zoom',
      'spin',
      'aberration',
    ])
    this.present = this.compile(presentSource, [
      'scene',
      'frame',
      'video',
      'resolution',
      'beat',
      'grain',
      'hits',
      'levels',
      'swell',
      'videoOn',
      'overlay',
      'videoAspect',
    ])
  }
  private compile(fragment: string, names: string[]): Program {
    const gl = this.gl
    const shaders: WebGLShader[] = []
    const program = gl.createProgram()
    if (!program) throw new Error('The graphics program could not start.')
    try {
      for (const [type, source] of [
        [gl.VERTEX_SHADER, vertex],
        [gl.FRAGMENT_SHADER, fragment],
      ] as const) {
        const shader = gl.createShader(type)
        if (!shader) throw new Error('The graphics shader could not start.')
        shaders.push(shader)
        gl.shaderSource(shader, source)
        gl.compileShader(shader)
        if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS))
          throw new Error(gl.getShaderInfoLog(shader) ?? 'Shader compilation failed.')
        gl.attachShader(program, shader)
      }
      gl.linkProgram(program)
      if (!gl.getProgramParameter(program, gl.LINK_STATUS))
        throw new Error(gl.getProgramInfoLog(program) ?? 'Shader linking failed.')
    } catch (error) {
      gl.deleteProgram(program)
      throw error
    } finally {
      for (const shader of shaders) gl.deleteShader(shader)
    }
    const uniforms: Uniforms = {}
    for (const name of names) uniforms[name] = gl.getUniformLocation(program, name)
    return { program, uniforms }
  }
  private getProgram(preset: VisualizerPresetId): Program {
    const cached = this.programs.get(preset)
    if (cached) {
      this.programs.delete(preset)
      this.programs.set(preset, cached)
      return cached
    }
    const entry = this.compile(fragmentSource(preset), sceneUniforms)
    this.programs.set(preset, entry)
    // Keep GPU memory bounded while allowing quick previous/next switching.
    if (this.programs.size > 4) {
      const oldest = this.programs.keys().next().value!
      this.gl.deleteProgram(this.programs.get(oldest)!.program)
      this.programs.delete(oldest)
    }
    return entry
  }
  private target(width: number, height: number, mipmaps: boolean): Target {
    const gl = this.gl
    const texture = gl.createTexture()
    const framebuffer = gl.createFramebuffer()
    if (!texture || !framebuffer) throw new Error('The graphics buffers could not start.')
    gl.bindTexture(gl.TEXTURE_2D, texture)
    const levels = mipmaps ? Math.floor(Math.log2(Math.max(width, height))) + 1 : 1
    gl.texStorage2D(gl.TEXTURE_2D, levels, gl.RGBA8, width, height)
    gl.texParameteri(
      gl.TEXTURE_2D,
      gl.TEXTURE_MIN_FILTER,
      mipmaps ? gl.LINEAR_MIPMAP_LINEAR : gl.LINEAR,
    )
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR)
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE)
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE)
    gl.bindFramebuffer(gl.FRAMEBUFFER, framebuffer)
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, texture, 0)
    gl.clearColor(0, 0, 0, 1)
    gl.clear(gl.COLOR_BUFFER_BIT)
    return { texture, framebuffer }
  }
  private releaseTargets(): void {
    for (const target of [this.scene, ...this.history]) {
      if (!target) continue
      this.gl.deleteTexture(target.texture)
      this.gl.deleteFramebuffer(target.framebuffer)
    }
    this.scene = null
    this.history = []
  }
  private ensureTargets(width: number, height: number): void {
    if (this.scene && this.size.width === width && this.size.height === height) return
    this.releaseTargets()
    this.scene = this.target(width, height, true)
    this.history = [this.target(width, height, false), this.target(width, height, false)]
    this.size = { width, height }
  }
  draw(
    signal: VisualizerSignal,
    settings: VisualizerSettings,
    palette: Palette,
    reducedMotion: boolean,
    reservedBottom = 0,
    video: VisualizerVideo | null = null,
  ): void {
    const gl = this.gl
    if (gl.isContextLost()) return
    const { width, height } = this.preview
      ? { width: this.canvas.width, height: this.canvas.height }
      : renderSize(
          this.canvas.clientWidth,
          this.canvas.clientHeight,
          window.devicePixelRatio || 1,
          settings.quality,
        )
    const scale = height / Math.max(1, this.canvas.clientHeight)
    if (this.canvas.width !== width || this.canvas.height !== height) {
      this.canvas.width = width
      this.canvas.height = height
    }
    this.ensureTargets(width, height)
    const beat = reducedMotion ? 0 : signal.beat
    const bottom = this.preview ? 0 : reservedBottom * scale
    const top = this.preview ? 0 : 65 * scale
    const visible = Math.max(1, height - bottom - top)
    gl.viewport(0, 0, width, height)

    const scene = this.getProgram(settings.preset)
    gl.bindFramebuffer(gl.FRAMEBUFFER, this.scene!.framebuffer)
    gl.useProgram(scene.program)
    let u = scene.uniforms
    gl.uniform2f(u.resolution!, width, height)
    gl.uniform2f(u.visualBounds!, bottom, visible)
    gl.uniform1fv(u['bands[0]']!, signal.bands)
    gl.uniform1fv(u['peaks[0]']!, signal.peaks)
    gl.uniform1f(u.time!, reducedMotion ? 0 : signal.timeSeconds)
    gl.uniform1f(u.intensity!, settings.intensity)
    gl.uniform4fv(u.levels!, signal.levels)
    // Reduced motion removes flashes, not the steady response to loudness.
    gl.uniform3fv(u.hits!, reducedMotion ? [0, 0, 0] : signal.hits)
    gl.uniform1f(u.swell!, signal.swell)
    gl.uniform3fv(u.colorA!, palette[0])
    gl.uniform3fv(u.colorB!, palette[1])
    gl.drawArrays(gl.TRIANGLES, 0, 3)
    gl.bindTexture(gl.TEXTURE_2D, this.scene!.texture)
    gl.generateMipmap(gl.TEXTURE_2D)

    // Styles are tuned at 60 fps. Scale them so 30 fps trails keep the same length.
    const style = feedbackStyles[settings.preset]
    const frames = 60 / settings.fps
    const [previous, next] = this.history as [Target, Target]
    gl.bindFramebuffer(gl.FRAMEBUFFER, next.framebuffer)
    gl.useProgram(this.feedback.program)
    u = this.feedback.uniforms
    gl.activeTexture(gl.TEXTURE0)
    gl.bindTexture(gl.TEXTURE_2D, this.scene!.texture)
    gl.activeTexture(gl.TEXTURE1)
    gl.bindTexture(gl.TEXTURE_2D, previous.texture)
    gl.uniform1i(u.scene!, 0)
    gl.uniform1i(u.history!, 1)
    gl.uniform2f(u.resolution!, width, height)
    gl.uniform2f(u.center!, 0.5, (bottom + visible / 2) / height)
    gl.uniform1f(u.decay!, Math.pow(style.decay, frames))
    if (reducedMotion) {
      gl.uniform2f(u.drift!, 0, 0)
      gl.uniform1f(u.zoom!, 1)
      gl.uniform1f(u.spin!, 0)
    } else {
      gl.uniform2f(u.drift!, style.drift[0] * frames, style.drift[1] * frames)
      gl.uniform1f(u.zoom!, Math.pow(style.zoom + beat * 0.006, frames))
      gl.uniform1f(u.spin!, style.spin * frames)
    }
    gl.uniform1f(u.aberration!, 0.002 + beat * 0.012)
    gl.drawArrays(gl.TRIANGLES, 0, 3)

    const showVideo = this.uploadVideo(video)
    gl.bindFramebuffer(gl.FRAMEBUFFER, null)
    gl.useProgram(this.present.program)
    u = this.present.uniforms
    gl.activeTexture(gl.TEXTURE1)
    gl.bindTexture(gl.TEXTURE_2D, next.texture)
    gl.activeTexture(gl.TEXTURE2)
    gl.bindTexture(gl.TEXTURE_2D, this.videoTexture)
    gl.uniform1i(u.scene!, 0)
    gl.uniform1i(u.frame!, 1)
    gl.uniform1i(u.video!, 2)
    gl.uniform2f(u.resolution!, width, height)
    gl.uniform1f(u.beat!, beat)
    gl.uniform1f(u.grain!, reducedMotion ? 0 : (signal.timeSeconds * 61.7) % 97)
    gl.uniform3fv(u.hits!, reducedMotion ? [0, 0, 0] : signal.hits)
    gl.uniform4fv(u.levels!, signal.levels)
    gl.uniform1f(u.swell!, signal.swell)
    gl.uniform1f(u.videoOn!, showVideo ? 1 : 0)
    gl.uniform1f(u.overlay!, settings.videoOverlay)
    gl.uniform1f(u.videoAspect!, video?.aspect ?? 16 / 9)
    gl.drawArrays(gl.TRIANGLES, 0, 3)
    gl.activeTexture(gl.TEXTURE0)
    this.history = [next, previous]
  }
  /** Copies the newest video frame into its texture. Returns whether a frame is available. */
  private uploadVideo(video: VisualizerVideo | null): boolean {
    if (!video || this.videoBlocked || !video.ready) return false
    const gl = this.gl
    if (!this.videoTexture) {
      this.videoTexture = gl.createTexture()
      if (!this.videoTexture) return false
      gl.activeTexture(gl.TEXTURE2)
      gl.bindTexture(gl.TEXTURE_2D, this.videoTexture)
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR)
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR)
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE)
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE)
      this.videoFilled = false
    }
    // A new texture needs a frame even if the element shows no new one.
    if (!video.takeFrame() && this.videoFilled) return true
    gl.activeTexture(gl.TEXTURE2)
    gl.bindTexture(gl.TEXTURE_2D, this.videoTexture)
    gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true)
    try {
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, video.element)
      this.videoFilled = true
    } catch {
      // A cross-origin refusal does not recover for this element. Keep the visualizer running.
      this.videoBlocked = true
    } finally {
      gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, false)
      gl.activeTexture(gl.TEXTURE0)
    }
    return this.videoFilled && !this.videoBlocked
  }
  dispose(): void {
    if (this.videoTexture) this.gl.deleteTexture(this.videoTexture)
    this.videoTexture = null
    for (const { program } of this.programs.values()) this.gl.deleteProgram(program)
    this.programs.clear()
    this.gl.deleteProgram(this.feedback.program)
    this.gl.deleteProgram(this.present.program)
    this.releaseTargets()
  }
}
