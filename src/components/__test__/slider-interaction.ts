interface SliderWrapper {
  element: Element;
  trigger(event: string, options: Record<string, number>): Promise<void>;
}

export async function dragSlider(
  slider: SliderWrapper,
  clientX: number,
): Promise<void> {
  const element = slider.element as HTMLElement;
  const capturedPointers = new Set<number>();

  Object.defineProperties(element, {
    getBoundingClientRect: {
      configurable: true,
      value: () =>
        ({
          bottom: 10,
          height: 10,
          left: 0,
          right: 100,
          top: 0,
          width: 100,
          x: 0,
          y: 0,
          toJSON: () => ({}),
        }) satisfies DOMRect,
    },
    hasPointerCapture: {
      configurable: true,
      value: (pointerId: number) => capturedPointers.has(pointerId),
    },
    releasePointerCapture: {
      configurable: true,
      value: (pointerId: number) => capturedPointers.delete(pointerId),
    },
    setPointerCapture: {
      configurable: true,
      value: (pointerId: number) => capturedPointers.add(pointerId),
    },
  });

  await slider.trigger("pointerdown", { clientX, pointerId: 1 });
  await slider.trigger("pointerup", { clientX, pointerId: 1 });
}
