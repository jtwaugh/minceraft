// Shared look for the DOM overlays: one palette and one button style. Plain
// values and a pure CSS generator; imports only ./panel.js, so it never
// pulls in wasm.
import { PANEL_TILE } from './panel.js';

/** Colours the overlays share; the values are the ones already in use across the client. */
export const PALETTE = {
  text: '#fff',
  textShadow: '#000',
  dim: 'rgba(0, 0, 0, 0.55)',
  dimStrong: 'rgba(0, 0, 0, 0.7)',
  deathDim: 'rgba(60, 0, 0, 0.5)',
  consoleEcho: '#9ad',
  consoleResult: '#fff',
  consoleError: '#f88',
};

/**
 * CSS for a button matching `selector` at UI `scale`: the raised panel as its
 * border image, the pressed panel on `:active`, a pointer cursor. The urls are
 * the panelImage data URLs for the 'raised' and 'pressed' states.
 */
export function buttonCss(selector, scale, raisedUrl, pressedUrl) {
  const width = `${PANEL_TILE * scale}px`;
  const base = `box-sizing: border-box; border-style: solid; border-width: ${width}; border-image-slice: ${PANEL_TILE}; border-image-repeat: stretch; image-rendering: pixelated;`;
  return [
    `${selector} { ${base} border-image-source: url(${raisedUrl}); background: none; font: inherit; color: ${PALETTE.text}; text-shadow: 0 0 3px ${PALETTE.textShadow}; cursor: pointer; }`,
    `${selector}:active { border-image-source: url(${pressedUrl}); }`,
  ].join('\n');
}
