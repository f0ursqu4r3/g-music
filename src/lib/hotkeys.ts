export type PlaybackHotkey = "toggle" | "previous" | "next" | "toggleMute";

export function resolvePlaybackHotkey(
  key: string,
  isTextEditing: boolean,
  hasModifier = false,
): PlaybackHotkey | null {
  if (isTextEditing || hasModifier) {
    return null;
  }

  switch (key.toLowerCase()) {
    case " ":
      return "toggle";
    case "j":
      return "previous";
    case "k":
      return "next";
    case "m":
      return "toggleMute";
    default:
      return null;
  }
}
