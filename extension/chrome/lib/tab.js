// The game tab through the Chrome DevTools Protocol: screenshots, and
// clicks the page cannot tell from real ones (a cloud phone stream ignores
// synthetic DOM events).  Attaching shows Chrome's "started debugging this
// browser" bar; detaching removes it.

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export class GameTab {
  constructor(tabId) {
    this.target = { tabId };
    this.attached = false;
  }

  async attach() {
    if (this.attached) return;
    try {
      await chrome.debugger.attach(this.target, '1.3');
    } catch (err) {
      if (!/already attached/i.test(err.message)) throw err;
    }
    this.attached = true;
    await sleep(400); // the debugging bar resizes the page as it appears
  }

  async detach() {
    if (!this.attached) return;
    this.attached = false;
    try {
      await chrome.debugger.detach(this.target);
    } catch {
      // already gone (tab closed, or the bar's Cancel was pressed)
    }
  }

  send(method, params = {}) {
    return chrome.debugger.sendCommand(this.target, method, params);
  }

  /**
   * The visible page as ImageData, the device pixels per CSS pixel (to turn
   * image positions into click positions), and the PNG as base64.
   */
  async capture() {
    const { data } = await this.send('Page.captureScreenshot', { format: 'png' });
    const metrics = await this.send('Page.getLayoutMetrics');
    const bytes = Uint8Array.from(atob(data), (c) => c.charCodeAt(0));
    const bitmap = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const ctx = canvas.getContext('2d', { willReadFrequently: true });
    ctx.drawImage(bitmap, 0, 0);
    const img = ctx.getImageData(0, 0, bitmap.width, bitmap.height);
    const cssWidth = (metrics.cssVisualViewport || metrics.layoutViewport).clientWidth;
    return { img, ratio: bitmap.width / cssWidth, png: data };
  }

  /** Click at a CSS-pixel position, as a mouse or as a finger. */
  async click(x, y, method = 'mouse') {
    if (method === 'touch') {
      await this.send('Input.dispatchTouchEvent', { type: 'touchStart', touchPoints: [{ x, y }] });
      await sleep(60);
      await this.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [] });
      return;
    }
    await this.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
    await this.send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 });
    await sleep(60);
    await this.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 });
  }
}
