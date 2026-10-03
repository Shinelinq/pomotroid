import test from 'node:test';
import assert from 'node:assert/strict';
import { miniPointer } from '../../src/lib/mini/pointer.ts';

test('locked mini blocks only drag and unlocks immediately; controls never start dragging', async () => {
  const oldWindow = globalThis.window,
    oldElement = globalThis.Element;
  class Element {
    constructor(button = false) {
      this.button = button;
    }
    closest() {
      return this.button;
    }
  }
  globalThis.Element = Element;
  const handlers = new Map();
  globalThis.window = { addEventListener() {}, removeEventListener() {} };
  const node = {
    addEventListener: (key, fn) => handlers.set(key, fn),
    removeEventListener() {},
    setPointerCapture() {},
    hasPointerCapture: () => false,
  };
  let locked = true,
    drags = 0,
    restores = 0,
    menus = 0;
  const binding = miniPointer(node, {
    locked: () => locked,
    drag: async () => {
      drags++;
    },
    restore: () => restores++,
    menu: () => menus++,
  });
  const event = (x = 0, target = new Element()) => ({
    button: 0,
    buttons: 1,
    clientX: x,
    clientY: 0,
    pointerId: 1,
    target,
    preventDefault() {},
  });
  try {
    handlers.get('pointerdown')(event());
    handlers.get('pointermove')(event(20));
    assert.equal(drags, 0);
    handlers.get('dblclick')(event());
    handlers.get('contextmenu')(event());
    assert.equal(restores, 1);
    assert.equal(menus, 1);
    locked = false;
    handlers.get('pointerdown')(event(0, new Element(true)));
    handlers.get('pointermove')(event(20));
    assert.equal(drags, 0);
    handlers.get('pointerdown')(event());
    handlers.get('pointermove')(event(3));
    assert.equal(drags, 0);
    handlers.get('pointermove')(event(8));
    assert.equal(drags, 1);
  } finally {
    binding.destroy();
    globalThis.window = oldWindow;
    globalThis.Element = oldElement;
  }
});
