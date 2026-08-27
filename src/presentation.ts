export interface PlayerDimensions {
  width: number;
  height: number;
}

export function playerDimensions(queueExpanded: boolean): PlayerDimensions {
  return {
    width: 520,
    height: queueExpanded ? 420 : 174,
  };
}
