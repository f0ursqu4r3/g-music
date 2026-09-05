export interface PlayerDimensions {
  width: number
  height: number
}

export function playerDimensions(queueExpanded: boolean): PlayerDimensions {
  return {
    width: 480,
    height: queueExpanded ? 390 : 144,
  }
}
