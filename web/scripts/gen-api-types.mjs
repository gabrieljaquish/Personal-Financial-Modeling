#!/usr/bin/env node
/*
 * API types for the front end, generated from the server's OpenAPI snapshot.
 *
 *   crates/pfp-server/tests/snapshots/openapi__openapi_v1.snap  ->  web/src/api/schema.gen.ts
 *
 * The chain of custody this gives (DECISIONS.md ADR-017: "a DTO change breaks the
 * front-end build, not a user session"): a Rust DTO change fails the insta
 * snapshot; accepting the snapshot fails tests/api-types-drift.test.mjs; running
 * `npm run gen:api` changes the committed types; `tsc` then fails wherever the UI
 * used the old shape.
 *
 * Deliberate choices:
 *
 *   1. No dependency. A schema-to-types package would multiply the installed tree,
 *      every member of which must clear the exact-SPDX licence gate.
 *   2. Fail closed. The generator supports exactly the JSON Schema keywords the
 *      snapshot uses. An unknown keyword THROWS; it is never skipped, because a
 *      skipped keyword is a constraint the types silently do not express.
 *   3. Money is opaque. A schema marked `x-money` becomes an object type with a
 *      unique-symbol brand, so `+ - * /` on an amount is a compile error. (A
 *      relational operator between two amounts is NOT a type error in TypeScript;
 *      tests/no-storage.test.mjs closes that with a source lint.)
 *   4. Comment text is neutralised. The output lives in web/src, where a URL-scheme
 *      literal is banned (the server's CSP derivation refuses a bundle that names
 *      one) - so a scheme in a description loses its scheme, and a comment
 *      terminator can never close a comment early.
 *   5. Deterministic output: snapshot order (sorted), LF, one trailing newline, no
 *      timestamp and no hash.
 *
 * Usage: npm run gen:api
 */

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const WEB_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..');

export const SNAPSHOT_RELATIVE = 'crates/pfp-server/tests/snapshots/openapi__openapi_v1.snap';
export const SNAPSHOT_PATH = join(WEB_ROOT, '..', ...SNAPSHOT_RELATIVE.split('/'));
export const OUTPUT_PATH = join(WEB_ROOT, 'src', 'api', 'schema.gen.ts');

const REF_PREFIX = '#/components/schemas/';

/** The body of an insta snapshot: everything after the second `---` line. */
export function snapshotBody(text) {
  const lines = text.split('\n');
  const end = lines[0] === '---' ? lines.indexOf('---', 1) : -1;
  if (end === -1) {
    throw new Error('not an insta snapshot: front matter not found');
  }
  return lines.slice(end + 1).join('\n');
}

/** Makes description text safe inside a block comment in web/src. */
export function neutralise(text) {
  return text.replace(/https?:\/\//gi, '').replace(/https?:/gi, '').replaceAll('*/', '* /');
}

function jsdoc(description, notes, indent) {
  const lines = [...(description === undefined ? [] : neutralise(description).split('\n')), ...notes];
  if (lines.length === 0) {
    return '';
  }
  const body = lines.map((line) => `${indent} *${line === '' ? '' : ` ${line.trimEnd()}`}`).join('\n');
  return `${indent}/**\n${body}\n${indent} */\n`;
}

function only(node, allowed, where) {
  for (const key of Object.keys(node)) {
    if (!allowed.includes(key)) {
      throw new Error(`unsupported keyword "${key}" at ${where}`);
    }
  }
}

function refName(ref, where) {
  if (typeof ref !== 'string' || !ref.startsWith(REF_PREFIX)) {
    throw new Error(`unsupported $ref at ${where}`);
  }
  const name = ref.slice(REF_PREFIX.length);
  if (!/^[A-Za-z][A-Za-z0-9]*$/.test(name)) {
    throw new Error(`unsupported schema name at ${where}`);
  }
  return name;
}

function literal(value, where) {
  if (typeof value !== 'string' || !/^[A-Za-z0-9._-]+$/.test(value)) {
    throw new Error(`unsupported enum value at ${where}`);
  }
  return `'${value}'`;
}

const PRIMITIVES = { integer: 'number', boolean: 'boolean' };

/** The TypeScript type of a schema used in a property, item or map-value position. */
function tsType(schema, where) {
  if ('x-money' in schema) {
    throw new Error(`inline x-money at ${where}: money must be a named schema`);
  }
  if ('$ref' in schema) {
    only(schema, ['$ref', 'description'], where);
    return refName(schema.$ref, where);
  }
  if ('oneOf' in schema) {
    only(schema, ['oneOf', 'description'], where);
    const members = schema.oneOf;
    const nulls = members.filter((m) => m.type === 'null' && Object.keys(m).length === 1);
    const refs = members.filter((m) => '$ref' in m);
    if (members.length !== 2 || nulls.length !== 1 || refs.length !== 1) {
      throw new Error(`unsupported oneOf at ${where}: only null + one $ref`);
    }
    return `${tsType(refs[0], where)} | null`;
  }
  if (Array.isArray(schema.type)) {
    // `["integer", "null"]`: one base type, nullable.
    const bases = schema.type.filter((t) => t !== 'null');
    if (bases.length !== 1 || schema.type.length !== 2) {
      throw new Error(`unsupported type list at ${where}: only one type plus null`);
    }
    const base = tsType({ ...schema, type: bases[0] }, where);
    return base.startsWith('readonly ') ? `(${base}) | null` : `${base} | null`;
  }
  const { type } = schema;
  switch (type) {
    case 'string':
      only(schema, ['type', 'description', 'format', 'minLength', 'maxLength', 'enum'], where);
      return 'enum' in schema ? schema.enum.map((v) => literal(v, where)).join(' | ') : 'string';
    case 'integer':
    case 'boolean':
      only(schema, ['type', 'description', 'format'], where);
      return PRIMITIVES[type];
    case 'array': {
      only(schema, ['type', 'description', 'items'], where);
      if (schema.items === undefined) {
        throw new Error(`array without items at ${where}`);
      }
      const item = tsType(schema.items, `${where}[]`);
      return item.includes(' ') ? `readonly (${item})[]` : `readonly ${item}[]`;
    }
    case 'object': {
      only(schema, ['type', 'description', 'additionalProperties', 'propertyNames'], where);
      const values = schema.additionalProperties;
      if (typeof values !== 'object' || values === null) {
        throw new Error(`inline object without a value schema at ${where}`);
      }
      if ('propertyNames' in schema) {
        only(schema.propertyNames, ['type'], `${where}/propertyNames`);
        if (schema.propertyNames.type !== 'string') {
          throw new Error(`unsupported propertyNames at ${where}`);
        }
      }
      return `Readonly<Record<string, ${tsType(values, `${where}{}`)}>>`;
    }
    default:
      throw new Error(`unsupported type "${String(type)}" at ${where}`);
  }
}

function lengthNotes(schema) {
  const notes = [];
  if ('minLength' in schema) {
    notes.push(`Minimum length: ${schema.minLength}.`);
  }
  if ('maxLength' in schema) {
    notes.push(`Maximum length: ${schema.maxLength}.`);
  }
  return notes;
}

function namedSchema(name, schema) {
  const where = `schemas/${name}`;
  if (schema['x-money'] === true) {
    only(schema, ['type', 'format', 'description', 'x-money'], where);
    if (schema.type !== 'integer') {
      throw new Error(`money that is not an integer at ${where}`);
    }
    const notes = ['', 'Opaque on purpose: arithmetic on an amount is a compile error. The only ways', 'across the boundary are in `format/money.ts`.'];
    return `${jsdoc(schema.description, notes, '')}export type ${name} = { readonly __cents: unique symbol };\n`;
  }
  if (schema.type === 'string' && 'enum' in schema) {
    only(schema, ['type', 'description', 'enum'], where);
    const values = schema.enum.map((v) => literal(v, where));
    return (
      `${jsdoc(schema.description, [], '')}export const ${name}Values = [${values.join(', ')}] as const;\n` +
      `export type ${name} = (typeof ${name}Values)[number];\n`
    );
  }
  if (schema.type !== 'object') {
    throw new Error(`unsupported named schema at ${where}`);
  }
  only(schema, ['type', 'description', 'properties', 'required', 'additionalProperties'], where);
  if ('additionalProperties' in schema && schema.additionalProperties !== false) {
    throw new Error(`unsupported additionalProperties at ${where}`);
  }
  const required = new Set(schema.required ?? []);
  const properties = Object.entries(schema.properties ?? {});
  for (const key of required) {
    if (!(key in (schema.properties ?? {}))) {
      throw new Error(`required property "${key}" is not declared at ${where}`);
    }
  }
  if (properties.length === 0) {
    return `${jsdoc(schema.description, [], '')}export type ${name} = Readonly<Record<string, never>>;\n`;
  }
  const body = properties
    .map(([key, property]) => {
      if (!/^[A-Za-z][A-Za-z0-9]*$/.test(key)) {
        throw new Error(`unsupported property name "${key}" at ${where}`);
      }
      const type = tsType(property, `${where}.${key}`);
      // Optional = not in `required`. The wire omits an absent value; a schema may
      // also say nullable; the type admits both.
      const optional = !required.has(key);
      const nullable = optional && !/(^|\| )null$/.test(type) ? `${type} | null` : type;
      return `${jsdoc(property.description, lengthNotes(property), '  ')}  readonly ${key}${optional ? '?' : ''}: ${nullable};\n`;
    })
    .join('');
  return `${jsdoc(schema.description, [], '')}export interface ${name} {\n${body}}\n`;
}

function bodyType(content, where) {
  if (content === undefined) {
    throw new Error(`a response without content at ${where}`);
  }
  const kinds = new Set();
  for (const [mime, media] of Object.entries(content)) {
    only(media, ['schema'], `${where}/${mime}`);
    const schema = media.schema;
    if ('$ref' in schema && mime === 'application/json') {
      kinds.add(`json:${refName(schema.$ref, where)}`);
    } else if (schema.type === 'string' && Object.keys(schema).length === 1) {
      kinds.add('text:string');
    } else {
      throw new Error(`unsupported response schema at ${where}/${mime}`);
    }
  }
  if (kinds.size !== 1) {
    throw new Error(`a response with mixed body kinds at ${where}`);
  }
  const [kind, type] = [...kinds][0].split(':');
  return { kind, type };
}

function operation(path, item) {
  const where = `paths${path}`;
  only(item, ['post'], where);
  const op = item.post;
  only(op, ['operationId', 'requestBody', 'responses', 'security', 'summary', 'description', 'tags'], where);
  let request = 'undefined';
  if (op.requestBody !== undefined) {
    only(op.requestBody, ['content', 'required', 'description'], `${where}/requestBody`);
    const media = op.requestBody.content?.['application/json'];
    if (media === undefined || Object.keys(op.requestBody.content).length !== 1 || op.requestBody.required !== true) {
      throw new Error(`unsupported request body at ${where}`);
    }
    only(media, ['schema'], `${where}/requestBody`);
    request = refName(media.schema.$ref, `${where}/requestBody`);
  }
  const ok = [];
  const errors = [];
  for (const [status, response] of Object.entries(op.responses)) {
    if (!/^[1-5][0-9]{2}$/.test(status)) {
      throw new Error(`unsupported status "${status}" at ${where}`);
    }
    only(response, ['content', 'description'], `${where}/${status}`);
    const body = bodyType(response.content, `${where}/${status}`);
    if (status.startsWith('2')) {
      ok.push({ status, ...body });
    } else {
      if (body.kind !== 'json' || body.type !== 'ErrorBody') {
        throw new Error(`a refusal that is not the ErrorBody at ${where}/${status}`);
      }
      errors.push(status);
    }
  }
  if (ok.length !== 1) {
    throw new Error(`expected exactly one success response at ${where}`);
  }
  const summary = jsdoc([op.summary, op.description].filter((t) => t !== undefined).join('\n\n') || undefined, [], '  ');
  return (
    `${summary}  readonly '${path}': {\n` +
    `    readonly request: ${request};\n` +
    `    readonly ok: ${ok[0].type};\n` +
    `    readonly okStatus: ${ok[0].status};\n` +
    `    readonly expect: '${ok[0].kind}';\n` +
    `    readonly errorStatuses: ${errors.length === 0 ? 'never' : errors.join(' | ')};\n` +
    `  };\n`
  );
}

/** The whole generated file for an OpenAPI document. */
export function generate(doc) {
  const schemas = Object.entries(doc.components?.schemas ?? {});
  const paths = Object.entries(doc.paths ?? {});
  for (const [path] of paths) {
    if (!/^\/api\/v1\/[a-z0-9/-]+$/.test(path)) {
      throw new Error(`unsupported path "${path}"`);
    }
  }
  const header =
    `// GENERATED FILE - do not edit. Regenerate with \`npm run gen:api\`.\n` +
    `// Source: ${SNAPSHOT_RELATIVE}\n` +
    `// Generator: web/scripts/gen-api-types.mjs. tests/api-types-drift.test.mjs fails when this file\n` +
    `// and the snapshot disagree.\n`;
  const operations = `/** Every API operation: request body, success body and statuses. All are POST. */\nexport interface Operations {\n${paths
    .map(([path, item]) => operation(path, item))
    .join('')}}\n`;
  const pathList = `export const API_PATHS = [\n${paths.map(([path]) => `  '${path}',\n`).join('')}] as const;\n`;
  return [header, ...schemas.map(([name, schema]) => namedSchema(name, schema)), operations, pathList].join('\n');
}

export function generateFromSnapshot(snapshotText) {
  return generate(JSON.parse(snapshotBody(snapshotText)));
}

if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(process.argv[1]).href) {
  mkdirSync(dirname(OUTPUT_PATH), { recursive: true });
  writeFileSync(OUTPUT_PATH, generateFromSnapshot(readFileSync(SNAPSHOT_PATH, 'utf8')));
  console.log(`wrote ${OUTPUT_PATH}`);
}
