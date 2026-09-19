// Renders real components to static markup under plain Node and parses the result.
// `renderToStaticMarkup` never runs effects, so what is rendered is exactly the
// view of the state handed in.

import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';

import { AnnounceContext, AppEnvContext } from '../../src/env.ts';
import { fakeApiEnv, fakeAppEnv } from './fakes.mjs';
import { parse } from './markup.mjs';

export function render(element, { env, announce = () => {} } = {}) {
  const appEnv = env ?? fakeAppEnv(fakeApiEnv(() => { throw new TypeError('no network in a render'); }).env).env;
  const markup = renderToStaticMarkup(
    createElement(AppEnvContext, { value: appEnv }, createElement(AnnounceContext, { value: announce }, element)),
  );
  return { markup, tree: parse(markup) };
}
