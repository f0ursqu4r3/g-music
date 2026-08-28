export const mockWindowViews = ["library", "artwork", "queue", "mini"] as const;

export type MockWindowView = (typeof mockWindowViews)[number];

export function resolveMockWindowView(search: string): MockWindowView {
  const view = new URLSearchParams(search).get("view");

  return mockWindowViews.includes(view as MockWindowView)
    ? (view as MockWindowView)
    : "library";
}
