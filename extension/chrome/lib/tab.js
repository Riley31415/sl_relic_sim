// The game tab: screenshots with chrome.tabs.captureVisibleTab, which copies
// what is already on screen, and clicks through the Chrome DevTools Protocol
// the page cannot tell from real ones (a cloud phone stream ignores synthetic
// DOM events).  The protocol's own screenshot makes the page redraw, which
// flickers over the phone's video, so it is only the fallback: when the game
// tab is not the one showing in its window (tab capture would photograph the
// other tab) or tab capture fails.  Attaching shows Chrome's "started
// debugging this browser" bar; detaching removes it.

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// chrome.tabs.captureVisibleTab allows 2 calls a second
const CAPTURE_GAP = 520;

export class GameTab {
  /** warn(text): told once when screenshots fall back to the debugger. */
  constructor(tabId, warn = () => {}) {
    this.target = { tabId };
    this.attached = false;
    this.warn = warn;
    this.lastCapture = 0;
    this.warned = false;
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
    const data = await this.screenshot();
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

  /** The visible page as base64 PNG: tab capture, else the debugger's. */
  async screenshot() {
    const tab = await chrome.tabs.get(this.target.tabId);
    let why = 'the game tab is not the one showing in its window';
    if (tab.active) {
      const wait = this.lastCapture + CAPTURE_GAP - Date.now();
      if (wait > 0) await sleep(wait);
      try {
        const url = await chrome.tabs.captureVisibleTab(tab.windowId, { format: 'png' });
        this.lastCapture = Date.now();
        return url.slice(url.indexOf(',') + 1);
      } catch (err) {
        this.lastCapture = Date.now();
        why = `tab capture failed (${err.message})`;
      }
    }
    if (!this.warned) {
      this.warned = true;
      this.warn(`Screenshots through the debugger instead - ${why}. The page may flicker while it runs.`);
    }
    const { data } = await this.send('Page.captureScreenshot', { format: 'png' });
    return data;
  }

  /** Click at a CSS-pixel position with the mouse. */
  async click(x, y) {
    await this.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
    await this.send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 });
    await sleep(60);
    await this.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 });
  }
}
