// The Assumptions Registry as display strings: every parameter with its source,
// as-of date, vintage, projection rule and rounding rule. Group order and
// membership come from the DTO; nothing is inferred from an id. Lookups, joins
// and `array.length` only - a count is a length, not money.

import type { AssumptionDto, AssumptionsResponse, Cents, IndexSeriesDto, RatioDto, SourceDto, VintageDto } from '../api/schema.gen.ts';
import { centsToText } from '../format/money.ts';
import { ratioToText } from '../format/ratio.ts';
import { roundingBasisLabel, roundingDirectionLabel } from '../format/text.ts';
import { detailHeadingId, hrefFor } from '../router/hash.ts';
import type { SortableRow } from '../table/sort.ts';
import { sortText } from '../table/sort.ts';
import { badgeFor, isLocked, isSettled, vintageStatusSentence } from './status.ts';
import type { BadgeVm } from './status.ts';

export type Entry = readonly [label: string, value: string];

export interface TableRowVm extends SortableRow {
  readonly href: string | null;
  readonly period: string;
  readonly asOf: string;
  readonly vintageShort: string;
  readonly lockText: string;
  readonly badge: BadgeVm;
  readonly projection: string;
  readonly rounding: string;
  readonly sourceCount: string;
  readonly openItemCount: string;
}

export interface SeriesRowVm extends SortableRow {
  readonly href: string | null;
  readonly name: string;
  readonly publisherId: string;
  readonly asOf: string;
  readonly yearsCovered: string;
  readonly badge: BadgeVm;
  readonly sourceCount: string;
  readonly openItemCount: string;
}

export interface GridVm {
  readonly caption: string;
  readonly rowHeader: string;
  readonly columns: readonly string[];
  readonly rows: readonly { readonly header: string; readonly cells: readonly string[] }[];
}

export interface SourceVm {
  readonly title: string;
  readonly entries: readonly Entry[];
  readonly url: string;
  readonly sha256: string;
}

export interface DetailVm {
  readonly id: string;
  readonly headingId: string;
  readonly kind: 'Parameter table' | 'Index series';
  readonly badge: BadgeVm;
  readonly identity: readonly Entry[];
  readonly projection: readonly Entry[];
  readonly rounding: readonly Entry[];
  readonly grids: readonly GridVm[];
  readonly sources: readonly SourceVm[];
  readonly openItems: readonly string[];
}

export interface VintageVm {
  readonly name: string;
  readonly statusLine: string;
  /** Verified and locked: the status line needs no warning mark. */
  readonly settled: boolean;
  readonly tables: readonly TableRowVm[];
  readonly series: readonly SeriesRowVm[];
  readonly details: readonly DetailVm[];
}

export interface RegistryVm {
  readonly vintages: readonly VintageVm[];
  /** Every id a route may name. */
  readonly ids: readonly string[];
  /** The tax years any table publishes values for, ascending. A lookup, not a rule. */
  readonly publishedYears: readonly string[];
}

const NONE = 'None';

/** `federal-2026@d547e4b85b26…`: the first twelve characters of the content hash. */
export function shortVintageId(contentId: string): string {
  const at = contentId.indexOf('@');
  if (at === -1) {
    return contentId;
  }
  const hash = contentId.slice(at + 1);
  const shown = hash.slice(0, 12);
  return `${contentId.slice(0, at + 1)}${shown}${shown === hash ? '' : '…'}`;
}

function text(value: string | number | null | undefined): string | null {
  return value === undefined || value === null ? null : String(value);
}

function present(entries: readonly (readonly [string, string | null])[]): Entry[] {
  return entries.filter((entry): entry is Entry => entry[1] !== null);
}

function amounts(values: readonly Cents[]): string[] {
  return values.map(centsToText);
}

function projectionSummary(table: AssumptionDto): string {
  const p = table.projection;
  const baseYear = text(p.baseYear);
  const base = baseYear !== null ? `base year ${baseYear}` : p.baseYearByComponent ? 'base year by component' : null;
  const lag = text(p.lagYears);
  return [p.rule, p.index ?? null, p.indexSeries ?? null, lag === null ? null : `lag ${lag}`, base].filter((part) => part !== null).join('; ');
}

function roundingSummary(table: AssumptionDto): string {
  const r = table.rounding;
  if (r === undefined || r === null) {
    return NONE;
  }
  const increment = r.increment !== undefined && r.increment !== null ? centsToText(r.increment) : r.incrementByKey ? 'increment varies by key' : 'increment not stated';
  return [increment, roundingDirectionLabel(r.direction), roundingBasisLabel(r.basis)].join('; ');
}

function tableRow(table: AssumptionDto, vintage: VintageDto): TableRowVm {
  const badge = badgeFor(table.verification);
  return {
    id: table.id,
    href: hrefFor({ screen: 'assumption', id: table.id }),
    period: table.period ?? 'Not stated',
    asOf: table.asOf,
    vintageShort: shortVintageId(table.vintageId),
    lockText: isLocked(vintage) ? `Locked as ${vintage.lockedId ?? ''}` : 'Unlocked',
    badge,
    projection: projectionSummary(table),
    rounding: roundingSummary(table),
    sourceCount: String(table.sources.length),
    openItemCount: String(table.openItems.length),
    sortKeys: { id: table.id, asOf: table.asOf, verification: badge.rank, openItems: table.openItems.length },
  };
}

function yearsCovered(years: readonly number[]): string {
  const first = years[0];
  const last = years[years.length - 1];
  if (first === undefined || last === undefined) {
    return NONE;
  }
  return first === last ? String(first) : `${String(first)} to ${String(last)}`;
}

function seriesRow(series: IndexSeriesDto): SeriesRowVm {
  const badge = badgeFor(series.verification);
  return {
    id: series.id,
    href: hrefFor({ screen: 'assumption', id: series.id }),
    name: series.indexSeries,
    publisherId: series.seriesId ?? 'Not stated',
    asOf: series.asOf,
    yearsCovered: yearsCovered(series.years),
    badge,
    sourceCount: String(series.sources.length),
    openItemCount: String(series.openItems.length),
    sortKeys: { id: series.id, asOf: series.asOf, verification: badge.rank, openItems: series.openItems.length },
  };
}

function sourceVm(source: SourceDto): SourceVm {
  return {
    title: source.title,
    url: source.url,
    sha256: source.sha256,
    entries: present([
      ['Publisher', source.publisher ?? null],
      ['Retrieved', source.retrieved],
      ['The source’s own as-of date', source.asOf ?? null],
      ['Where in the document', source.locator ?? null],
      ['Archived copy (path in the repository; not served by the application)', source.archive ?? null],
    ]),
  };
}

function componentColumns(components: readonly string[], width: number): string[] {
  if (components.length === 0) {
    return width === 1 ? ['Amount'] : Array.from({ length: width }, (_, index) => `Amount ${String(index + 1)}`);
  }
  return [...components];
}

/**
 * The number of cells in a grid, read from its first row. Every row of a table
 * the loader accepted has the same length; a ragged one would be a defect and is
 * rendered as it is, never trimmed.
 */
function widthOf(rows: readonly (readonly unknown[])[]): number {
  return rows[0]?.length ?? 0;
}

function valueGrids(table: AssumptionDto): GridVm[] {
  return Object.entries(table.values).map(([key, byYear]) => {
    const rows = Object.entries(byYear);
    return {
      caption: `Published values of ${table.id} for ${key}, by tax year`,
      rowHeader: 'Tax year',
      columns: componentColumns(table.components, widthOf(rows.map(([, cells]) => cells))),
      rows: rows.map(([year, cells]) => ({ header: year, cells: amounts(cells) })),
    };
  });
}

function ratesGrid(table: AssumptionDto): GridVm[] {
  const rows = Object.entries(table.rates);
  if (rows.length === 0) {
    return [];
  }
  const width = widthOf(rows.map(([, ladder]) => ladder));
  return [
    {
      caption: `Rates of ${table.id}, by tax year, lowest first`,
      rowHeader: 'Tax year',
      columns: Array.from({ length: width }, (_, index) => `Rate ${String(index + 1)}`),
      rows: rows.map(([year, ladder]) => ({ header: year, cells: ladder.map((ratio: RatioDto) => ratioToText(ratio)) })),
    },
  ];
}

function baseGrids(table: AssumptionDto): GridVm[] {
  const p = table.projection;
  const grids: GridVm[] = [];
  const base = Object.entries(p.baseValues);
  if (base.length !== 0) {
    grids.push({
      caption: `Statutory base values of ${table.id}, by key`,
      rowHeader: 'Key',
      columns: componentColumns(table.components, widthOf(base.map(([, cells]) => cells))),
      rows: base.map(([key, cells]) => ({ header: key, cells: amounts(cells) })),
    });
  }
  const years = Object.entries(p.baseYearByComponent ?? {});
  if (years.length !== 0) {
    grids.push({
      caption: `Statutory base year of ${table.id}, per component, by the first tax year it applies to`,
      rowHeader: 'From tax year',
      columns: componentColumns(table.components, widthOf(years.map(([, cells]) => cells))),
      rows: years.map(([year, cells]) => ({ header: year, cells: cells.map(String) })),
    });
  }
  return grids;
}

function tableDetail(table: AssumptionDto, vintage: VintageDto): DetailVm {
  const p = table.projection;
  const r = table.rounding ?? null;
  return {
    id: table.id,
    headingId: detailHeadingId(table.id),
    kind: 'Parameter table',
    badge: badgeFor(table.verification),
    identity: present([
      ['Parameter id', table.id],
      ['Period', table.period ?? 'Not stated'],
      ['As of (the law as in effect on this date)', table.asOf],
      ['Vintage', table.vintageId],
      ['Lock', isLocked(vintage) ? `Locked as ${vintage.lockedId ?? ''}` : 'Unlocked'],
      ['Amounts are served in', table.servedUnit],
      ['Components', table.components.length === 0 ? 'A single amount' : table.components.join(', ')],
      ['Sources', String(table.sources.length)],
      ['Open items', String(table.openItems.length)],
    ]),
    projection: present([
      ['Rule', p.rule],
      ['Index the law names', p.index ?? null],
      ['Archived series it resolves to', p.indexSeries ?? null],
      ['Lag, in years', text(p.lagYears)],
      ['Base year', text(p.baseYear) ?? (p.baseYearByComponent ? 'By component (table below)' : null)],
      ['First adjusted tax year', text(p.firstAdjustedYear)],
      ...Object.entries(p.derived).map(([key, d]) => [`Derived key ${key}`, `${String(d.multiplier.num)}/${String(d.multiplier.den)} of ${d.from}`] as const),
    ]),
    rounding:
      r === null
        ? []
        : present([
            ['Increment', r.increment === undefined || r.increment === null ? null : centsToText(r.increment)],
            ...Object.entries(r.incrementByKey ?? {}).map(([key, cents]) => [`Increment for ${key}`, centsToText(cents)] as const),
            ['Direction', roundingDirectionLabel(r.direction)],
            ['Applied to', roundingBasisLabel(r.basis)],
          ]),
    grids: [...valueGrids(table), ...ratesGrid(table), ...baseGrids(table)],
    sources: table.sources.map(sourceVm),
    openItems: table.openItems,
  };
}

function seriesDetail(series: IndexSeriesDto): DetailVm {
  return {
    id: series.id,
    headingId: detailHeadingId(series.id),
    kind: 'Index series',
    badge: badgeFor(series.verification),
    identity: present([
      ['Series id', series.id],
      ['Name tables refer to it by', series.indexSeries],
      ['Publisher’s id', series.seriesId ?? 'Not stated'],
      ['As of (data as published on this date)', series.asOf],
      ['Calendar years covered', yearsCovered(series.years)],
      ['Sources', String(series.sources.length)],
      ['Open items', String(series.openItems.length)],
    ]),
    projection: [],
    rounding: [],
    grids: [],
    sources: series.sources.map(sourceVm),
    openItems: series.openItems,
  };
}

export function registryVm(response: AssumptionsResponse): RegistryVm {
  const vintages = response.vintages.map((vintage) => ({
    name: vintage.name,
    statusLine: vintageStatusSentence(vintage),
    settled: isSettled(vintage),
    tables: vintage.tables.map((table) => tableRow(table, vintage)),
    series: vintage.indexSeries.map(seriesRow),
    details: [...vintage.tables.map((table) => tableDetail(table, vintage)), ...vintage.indexSeries.map(seriesDetail)],
  }));
  const years = new Set<string>();
  for (const vintage of response.vintages) {
    for (const table of vintage.tables) {
      for (const byYear of Object.values(table.values)) {
        for (const year of Object.keys(byYear)) {
          years.add(year);
        }
      }
    }
  }
  return {
    vintages,
    ids: vintages.flatMap((vintage) => vintage.details.map((detail) => detail.id)),
    publishedYears: sortText([...years]),
  };
}
