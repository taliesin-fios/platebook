#!/usr/bin/env node
// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * Checks that every icon-only <button> in the client has an accessible name.
 * A button whose visible content has no words (an SVG, an emoji, "×", "+",
 * "‹") must have `aria-label` (or `aria-labelledby`); a `title` alone isn't
 * enough, since touch devices and many screen readers don't use it.
 * The client's `npm run lint` (and so CI) runs it.
 *
 *   node scripts/icon-button-labels.mjs
 *
 * Buttons whose content comes from an expression (`{label}`) count as having
 * words: the check can't see what they render, so keep those to text.
 */
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const clientDir = path.join(root, 'client');
const ts = createRequire(path.join(clientDir, 'package.json'))('typescript');

function tsxFiles(dir) {
  const out = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, entry.name);
    if (entry.isDirectory()) out.push(...tsxFiles(p));
    else if (p.endsWith('.tsx') && !p.endsWith('.test.tsx')) out.push(p);
  }
  return out;
}

/** The button's visible words, or null when part of it is an expression we can't read. */
function visibleText(children) {
  let text = '';
  let dynamic = false;
  const expr = e => {
    if (!e || e.kind === ts.SyntaxKind.NullKeyword) return;
    if (ts.isStringLiteral(e) || ts.isNoSubstitutionTemplateLiteral(e)) text += e.text;
    else if (ts.isParenthesizedExpression(e)) expr(e.expression);
    else if (ts.isConditionalExpression(e)) { expr(e.whenTrue); expr(e.whenFalse); }
    else if (ts.isBinaryExpression(e) && e.operatorToken.kind === ts.SyntaxKind.AmpersandAmpersandToken) expr(e.right);
    else if (ts.isJsxElement(e) || ts.isJsxSelfClosingElement(e) || ts.isJsxFragment(e)) jsx(e);
    else dynamic = true; // a variable, call or template: can't tell what it renders
  };
  const jsx = node => {
    if (ts.isJsxText(node)) text += node.text;
    else if (ts.isJsxExpression(node)) expr(node.expression);
    else if (ts.isJsxFragment(node)) node.children.forEach(jsx);
    else if (ts.isJsxElement(node)) {
      if (node.openingElement.tagName.getText() !== 'svg') node.children.forEach(jsx);
    } else if (ts.isJsxSelfClosingElement(node)) {
      // <SendIcon /> is an icon (and <span /> is empty); any other component may render text.
      const tag = node.tagName.getText();
      if (!/Icon$/.test(tag) && !/^[a-z]/.test(tag)) dynamic = true;
    }
  };
  children.forEach(jsx);
  return dynamic && !/[A-Za-z]{2,}/.test(text) ? null : text;
}

const problems = [];
for (const file of tsxFiles(path.join(clientDir, 'src'))) {
  const src = ts.createSourceFile(file, fs.readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const visit = node => {
    if (ts.isJsxElement(node) && node.openingElement.tagName.getText() === 'button') {
      const attrs = node.openingElement.attributes.properties
        .filter(a => ts.isJsxAttribute(a))
        .map(a => a.name.getText());
      const named = attrs.includes('aria-label') || attrs.includes('aria-labelledby');
      const text = visibleText(node.children);
      if (!named && text !== null && !/[A-Za-z]{2,}/.test(text)) {
        const { line } = src.getLineAndCharacterOfPosition(node.getStart());
        const shown = text.replace(/\s+/g, ' ').trim() || 'icon';
        problems.push(`${path.relative(root, file)}:${line + 1}: icon-only button (${shown}) needs aria-label`);
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(src);
}

if (problems.length) {
  console.error(problems.join('\n'));
  console.error(`\n${problems.length} icon-only button(s) without aria-label.`);
  process.exit(1);
}
console.log('Icon buttons: all labelled.');
