/** Apply SVG size changes on the next frame, outside ResizeObserver delivery. */
export function measureWidth(node: HTMLElement, onWidth: (width: number) => void) {
  let frame = 0;
  let previous = -1;
  const observer = new ResizeObserver(([entry]) => {
    const width = entry.contentRect.width;
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => {
      if (width !== previous) {
        previous = width;
        onWidth(width);
      }
    });
  });
  observer.observe(node);
  return {
    destroy() {
      observer.disconnect();
      cancelAnimationFrame(frame);
    },
  };
}
