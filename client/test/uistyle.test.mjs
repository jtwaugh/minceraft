import test from 'node:test';
import assert from 'node:assert/strict';
import { PALETTE, buttonCss, panelCss } from '../uistyle.js';
import { PANEL_TILE } from '../panel.js';

test('button CSS scales its border width with the UI scale', () => {
  for (const scale of [1, 2, 5]) {
    const css = buttonCss('.btn', scale, 'raised.png', 'pressed.png');
    assert.match(css, new RegExp(`border-width: ${PANEL_TILE * scale}px`));
    assert.match(css, new RegExp(`border-image-slice: ${PANEL_TILE};`));
  }
});

test('button CSS names the raised and pressed states', () => {
  const [base, active] = buttonCss('.btn', 2, 'raised.png', 'pressed.png').split('\n');
  assert.match(base, /^\.btn \{.*url\(raised\.png\).*cursor: pointer/);
  assert.match(active, /^\.btn:active \{.*url\(pressed\.png\)/);
});

test('palette values are non-empty strings', () => {
  for (const v of Object.values(PALETTE)) assert.equal(typeof v, 'string');
});

test('panel CSS uses the panel border image and the shared dim colour', () => {
  const css = panelCss('#box', 2, 'p.png');
  assert.match(css, /^#box \{/);
  assert.match(css, new RegExp(`border-width: ${PANEL_TILE * 2}px`));
  assert.match(css, /border-image: url\(p\.png\) 4 stretch/);
  assert.ok(css.includes(`background: ${PALETTE.dim}`));
});
