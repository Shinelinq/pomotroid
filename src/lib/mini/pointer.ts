/** Only non-control regions drag; CSS pixels are logical pixels at every Windows DPI. */
export function miniPointer(
  node: HTMLElement,
  actions: {
    locked?: () => boolean;
    drag: () => Promise<void>;
    restore: () => void;
    menu: () => void;
  }
) {
  let origin: { x: number; y: number; id: number } | null = null;
  let suppressUntil = 0;
  const control = (target: EventTarget | null) =>
    target instanceof Element && !!target.closest('button');
  function down(event: PointerEvent) {
    if (actions.locked?.() || event.button !== 0 || control(event.target)) return;
    origin = { x: event.clientX, y: event.clientY, id: event.pointerId };
    node.setPointerCapture(event.pointerId);
  }
  function cancel() {
    const previous = origin;
    origin = null;
    if (previous && node.hasPointerCapture(previous.id)) node.releasePointerCapture(previous.id);
  }
  function move(event: PointerEvent) {
    if (actions.locked?.() || event.buttons !== 1) {
      cancel();
      return;
    }
    if (!origin || Math.hypot(event.clientX - origin.x, event.clientY - origin.y) < 4) return;
    cancel();
    suppressUntil = Infinity;
    void actions.drag().finally(() => {
      suppressUntil = performance.now() + 250;
    });
  }
  function click(event: MouseEvent) {
    if (performance.now() < suppressUntil) {
      event.preventDefault();
      event.stopImmediatePropagation();
    }
  }
  function doubleClick(event: MouseEvent) {
    if (control(event.target) || performance.now() < suppressUntil) return;
    event.preventDefault();
    actions.restore();
  }
  function menu(event: MouseEvent) {
    event.preventDefault();
    cancel();
    actions.menu();
  }
  node.addEventListener('pointerdown', down);
  node.addEventListener('pointermove', move);
  node.addEventListener('pointerup', cancel);
  node.addEventListener('pointercancel', cancel);
  node.addEventListener('lostpointercapture', cancel);
  window.addEventListener('blur', cancel);
  node.addEventListener('click', click, true);
  node.addEventListener('dblclick', doubleClick);
  node.addEventListener('contextmenu', menu);
  return {
    destroy() {
      cancel();
      node.removeEventListener('pointerdown', down);
      node.removeEventListener('pointermove', move);
      node.removeEventListener('pointerup', cancel);
      node.removeEventListener('pointercancel', cancel);
      node.removeEventListener('lostpointercapture', cancel);
      window.removeEventListener('blur', cancel);
      node.removeEventListener('click', click, true);
      node.removeEventListener('dblclick', doubleClick);
      node.removeEventListener('contextmenu', menu);
    },
  };
}
