// The validation report as display strings (TESTING.md §13; PLAN.md §4.13 item
// 8). Lookups and joins over the report the server sent; every number shown is
// a count the report carries, rendered with `String()`. Nothing here is money,
// and nothing is a target: a section the tree does not hold yet is shown as
// "not yet introduced" with its milestone, never dropped, and the headline is
// computed from the counts so it cannot be typed to today's zeros.

import type { ApiFailure } from '../api/errors.ts';
import type { SectionStatus, TierReport, ValidationReport, ValidationReportResponse, VintageReport } from '../api/schema.gen.ts';
import { listInWords } from '../format/text.ts';
import type { Loadable } from './status.ts';

export type Entry = readonly [label: string, value: string];

export interface TableVm {
  readonly caption: string;
  readonly captionId: string;
  readonly columns: readonly string[];
  readonly rows: readonly { readonly header: string; readonly cells: readonly string[] }[];
}

export interface ReportVm {
  /** The first sentence of the page: the counts, said plainly. */
  readonly headline: string;
  /** What the tree holds, table by table; empty when there is no report to show. */
  readonly tables: readonly TableVm[];
  readonly pins: readonly Entry[];
  /** The report's own statement of how it was computed, and its date if it has one. */
  readonly basis: readonly Entry[];
  /** The failure to offer a retry for, if the read failed. */
  readonly failure: ApiFailure | null;
}

const NO_REPORT = 'This build carries no validation report: it was made without running the report generator, so nothing on this page is validated and nothing computed by this build is for decisions.';
const UNREAD = 'The validation report has not been read yet; until it is, treat nothing as validated.';
const UNREADABLE = 'The validation report could not be read; treat nothing as validated.';

function plural(count: number, one: string, many: string): string {
  return `${String(count)} ${count === 1 ? one : many}`;
}

function countOf(counts: readonly { readonly name: string; readonly count: number }[], name: string): number {
  return counts.find((c) => c.name === name)?.count ?? 0;
}

function tier(report: ValidationReport, name: string): TierReport | undefined {
  return report.fixtures.tiers.find((t) => t.tier === name);
}

function vintageWords(vintage: VintageReport): string {
  return `parameter vintage ${vintage.name} ${vintage.locked ? 'locked' : 'unlocked'} and ${vintage.verified ? 'verified' : 'pending human verification'}`;
}

/** The truth about the tree, from its counts. */
export function headlineOf(report: ValidationReport): string {
  const tier1 = tier(report, 'tier1')?.fileCount ?? 0;
  const pending = report.unverified.pendingFixtureCount;
  const parts = [plural(tier1, 'tier-1 fixture', 'tier-1 fixtures'), `${plural(pending, 'fixture', 'fixtures')} pending human verification`];
  if (report.parameters.vintages.length === 0) {
    parts.push('no parameter vintage');
  }
  for (const vintage of report.parameters.vintages) {
    parts.push(vintageWords(vintage));
  }
  return `${parts.join('; ')}.`;
}

const STATES: Readonly<Record<string, (milestone: string) => string>> = {
  'not-yet-introduced': (m) => `Not yet introduced (milestone ${m})`,
  empty: (m) => `Empty (specified for milestone ${m})`,
  partial: (m) => `Partly present (milestone ${m})`,
  present: (m) => `Present (milestone ${m})`,
};

/** A section's state in words. An unknown state is said to be unknown, never upgraded. */
export function stateWords(status: SectionStatus): string {
  const words = Object.hasOwn(STATES, status.state) ? STATES[status.state] : undefined;
  return words === undefined ? `Unknown state "${status.state}" - treat as not present (milestone ${status.milestone})` : words(status.milestone);
}

const VERIFICATIONS = ['primary-source-confirmed', 'hand-worked-reviewed', 'pending-hand-verification'] as const;

function otherVerificationCount(t: TierReport): number {
  // A count of counts, not an amount: the documents whose verification value is
  // none of the three named ones.
  return t.byVerification.filter((c) => !(VERIFICATIONS as readonly string[]).includes(c.name)).reduce((total, c) => total + c.count, 0);
}

function tiersTable(report: ValidationReport): TableVm {
  return {
    caption: 'Fixtures by tier and verification value',
    captionId: 'about-fixtures-caption',
    columns: ['Tier', 'Status', 'Documents', 'Primary-source-confirmed', 'Hand-worked-reviewed', 'Pending hand verification', 'Other'],
    rows: report.fixtures.tiers.map((t) => ({
      header: t.tier,
      cells: [
        stateWords(t.status),
        String(t.fileCount),
        String(countOf(t.byVerification, 'primary-source-confirmed')),
        String(countOf(t.byVerification, 'hand-worked-reviewed')),
        String(countOf(t.byVerification, 'pending-hand-verification')),
        String(otherVerificationCount(t)),
      ],
    })),
  };
}

function milestonesTable(report: ValidationReport): TableVm {
  const rows = report.fixtures.tiers.flatMap((t) =>
    t.byMilestoneAndVerification.map((group) => ({
      header: `${t.tier}, ${group.milestone}`,
      cells: [group.verification, String(group.count)],
    })),
  );
  return {
    caption: 'Fixtures by tier, milestone and verification value',
    captionId: 'about-milestones-caption',
    columns: ['Tier and milestone', 'Verification', 'Documents'],
    rows: rows.length === 0 ? [{ header: 'none', cells: ['-', '0'] }] : rows,
  };
}

function vintagesTable(report: ValidationReport): TableVm {
  const rows = report.parameters.vintages.map((v) => ({
    header: v.name,
    cells: [
      v.locked ? 'yes' : 'no',
      v.verified ? 'yes' : 'no',
      String(v.tableCount),
      String(v.indexSeriesCount),
      String(v.openItemCount),
      `${String(v.archive.checksumMatchCount)} of ${String(v.archive.archivedCount)} match; ${String(v.archive.checksumMismatchCount)} differ; ${String(v.archive.missingCount)} missing`,
    ],
  }));
  return {
    caption: 'Parameter vintages',
    captionId: 'about-vintages-caption',
    columns: ['Vintage', 'Locked', 'Signed off by a person', 'Tables', 'Index series', 'Open questions', 'Archived sources checked'],
    rows: rows.length === 0 ? [{ header: 'none', cells: ['no', 'no', '0', '0', '0', '-'] }] : rows,
  };
}

function sectionsTable(report: ValidationReport): TableVm {
  const sections: readonly (readonly [string, SectionStatus])[] = [
    ['Tier-1 fixtures', tier(report, 'tier1')?.status ?? report.fixtures.status],
    ['Tier-2 pinned suites', report.tier2Suites],
    ['Tier-3 goldens', report.tier3Goldens],
    ['Recorded oracles', report.oracles],
    ['Property-based invariants', report.properties.status],
    ['Contract snapshots', report.contractSnapshots.status],
    ['Mutation score', report.mutationScore],
    ['Fuzz corpora', report.fuzzCorpora],
    ['Security suite', report.securitySuite.status],
    ['Browser end-to-end', report.browserEndToEnd],
    ['Performance budgets', report.performanceBudgets.status],
  ];
  return {
    caption: 'Corpora by section: what exists at this commit, and what the design places later',
    captionId: 'about-sections-caption',
    columns: ['Section', 'Status', 'Specified in', 'What is and is not counted'],
    rows: sections.map(([name, status]) => ({ header: name, cells: [stateWords(status), status.reference, status.note] })),
  };
}

function unverifiedTable(report: ValidationReport): TableVm {
  return {
    caption: 'Still unverified: the open items of TESTING.md section 5.2, printed, not hidden',
    captionId: 'about-unverified-caption',
    columns: ['Item', 'Gate', 'Milestone'],
    rows: report.unverified.items.map((item) => ({ header: item.item, cells: [item.gate, item.milestone] })),
  };
}

function testsTable(report: ValidationReport): TableVm {
  return {
    caption: 'Tests declared in the source tree (declared, not run)',
    captionId: 'about-tests-caption',
    columns: ['Crate', 'Test functions'],
    rows: [...report.testInventory.byCrate.map((c) => ({ header: c.name, cells: [String(c.count)] })), { header: 'web (node:test)', cells: [String(report.testInventory.webTestCount)] }],
  };
}

function pinsOf(report: ValidationReport): Entry[] {
  const p = report.pins;
  return [
    ['Application version the report was generated for', p.applicationVersion],
    ['Licence', p.licence],
    ['Engine version', p.engineVersion ?? 'none yet'],
    ['Schema version', p.schemaVersion ?? 'none yet'],
    ['Parameter vintage content ids', p.paramVintageContentIds.length === 0 ? 'none' : listInWords(p.paramVintageContentIds)],
    ['Locked parameter vintage ids', p.lockedParamVintageIds.length === 0 ? 'none' : listInWords(p.lockedParamVintageIds)],
    ['Binary digest', p.binaryDigest ?? 'none yet'],
    ['Why the absent pins are absent', p.note],
  ];
}

/** The page's view of a loaded (or not) report. */
export function reportVm(loadable: Loadable<ValidationReportResponse>): ReportVm {
  if (loadable.kind === 'failed') {
    return { headline: UNREADABLE, tables: [], pins: [], basis: [], failure: loadable.failure };
  }
  if (loadable.kind !== 'ready') {
    return { headline: UNREAD, tables: [], pins: [], basis: [], failure: null };
  }
  const { report } = loadable.value;
  if (loadable.value.state !== 'generated' || report === undefined || report === null) {
    return { headline: NO_REPORT, tables: [], pins: [], basis: [], failure: null };
  }
  return {
    headline: headlineOf(report),
    tables: [tiersTable(report), milestonesTable(report), vintagesTable(report), sectionsTable(report), unverifiedTable(report), testsTable(report)],
    pins: pinsOf(report),
    basis: [
      ['Generated on', report.generatedOn ?? 'no date: the generator reads no clock, and none was given'],
      ['How it was computed', report.basis],
      ['Tier-1 fixtures by kind', `${String(report.unverified.tier1PrimarySourceConfirmedCount)} primary-source-confirmed; ${String(report.unverified.tier1HandWorkedReviewedCount)} hand-worked-reviewed`],
      ['Parameter documents pending a human read-back', String(report.unverified.pendingParameterDocumentCount)],
    ],
    failure: null,
  };
}
