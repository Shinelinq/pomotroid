/** Observe the flex-allocated box, never the SVG's intrinsic size. */
export function measurePlot(
  node: HTMLElement,
  changed: (size: { width: number; height: number }) => void
) {
  let frame = 0;
  let lastWidth = -1;
  let lastHeight = -1;
  const observer = new ResizeObserver(([entry]) => {
    const width = Math.floor(entry.contentRect.width * 100) / 100;
    const height = Math.floor(entry.contentRect.height * 100) / 100;
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(() => {
      if (width !== lastWidth || height !== lastHeight) {
        lastWidth = width;
        lastHeight = height;
        changed({ width, height });
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
