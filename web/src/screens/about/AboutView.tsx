import type { SessionStatus, ValidationReportResponse } from '../../api/schema.gen.ts';
import { FailureAlert } from '../../components/FailureAlert.tsx';
import tableStyles from '../../components/Table.module.css';
import { hrefFor } from '../../router/hash.ts';
import { aboutVm } from '../../viewmodel/status.ts';
import type { Loadable } from '../../viewmodel/status.ts';
import { reportVm } from '../../viewmodel/validation.ts';
import type { Entry, TableVm } from '../../viewmodel/validation.ts';
import { ScreenHeading } from '../ScreenHeading.tsx';
import screenStyles from '../Screen.module.css';
import styles from './About.module.css';

/**
 * What the user is told is out of scope: SECURITY.md section 2.3, restated row
 * by row. The design document is the source of record; this list points at it and
 * does not narrow it. Each label is the row's first cell as the document states
 * it (a trailing parenthetical aside), and tests/about.test.mjs reads the
 * document and fails when a row is added, removed or reordered there.
 */
export const OUT_OF_SCOPE: readonly Entry[] = [
  ['Malware running as the same user, or as root', 'It can read process memory, keystrokes and the decrypted plan; no user-space design defeats it.'],
  ['A malicious browser extension with all-sites access', 'It runs inside this application’s origin and can read what the screens show. Use a dedicated, clean browser profile.'],
  ['Memory forensics of an unlocked process; swap or hibernation images', 'Locking memory and zeroing secrets reduce but do not eliminate exposure; auto-lock and exit-after-lock shorten the exposure.'],
  ['Coercion', 'Out of scope by construction.'],
  ['An attacker with write access rolling the file back to an older authentic version', 'Each version’s authenticity is guaranteed; freshness is not. The generation counter catches accidental stale copies only.'],
  ['File-size traffic analysis beyond the 64 KiB bucket', 'Padding hides small differences, not order-of-magnitude plan size.'],
  ['macOS CrashReporter', 'On an abnormal termination macOS writes a diagnostic report the application cannot suppress, and Apple receives it when Analytics sharing is on. The application holds no secret in a recoverable state at abort time; turn Analytics sharing off if that matters to you.'],
  ['Residual metadata of the launch', 'Opening the browser is recorded in the system log, and the launch address reaches browser history for the instant before its fragment is cleared. The token is single-use with a 60-second life, so what survives is the fact and time of a launch, not a usable credential.'],
  ['Copies already made, and purge', 'Purging and re-keying reach the plan file and the backups beside it, never a Time Machine snapshot, a cloud-sync version history or a file already forwarded. On APFS neither can guarantee that overwritten blocks are unrecoverable.'],
  ['Decline mode', 'Without the trust anchor the application cannot prove it is the program answering on this port; the protection against another program squatting the port rests entirely on comparing the certificate fingerprint, which the unlock screen makes a required step.'],
];

function Entries({ entries }: { entries: readonly Entry[] }) {
  return (
    <dl className={styles.entries}>
      {entries.map(([label, value]) => (
        <div key={label}>
          <dt>{label}</dt>
          <dd className={styles.code}>{value}</dd>
        </div>
      ))}
    </dl>
  );
}

function ReportTable({ table }: { table: TableVm }) {
  return (
    <div className={tableStyles.scrollRegion} role="region" aria-labelledby={table.captionId} tabIndex={0}>
      <table className={tableStyles.table}>
        <caption id={table.captionId}>{table.caption}</caption>
        <thead>
          <tr>
            {table.columns.map((column) => (
              <th key={column} scope="col">
                {column}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {table.rows.map((row) => (
            <tr key={row.header}>
              <th scope="row" className={styles.code}>
                {row.header}
              </th>
              {row.cells.map((cell, index) => (
                <td key={table.columns[index + 1] ?? `extra-${String(index)}`} className={styles.code}>
                  {cell}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/**
 * The About page (PLAN.md section 4.1): the honest headline first, then the
 * build's identity, the validation report as tables, and what is out of scope.
 * The persistent parameter banner is the shell's and is above every screen.
 */
export function AboutView({ status, report, onRetry }: { status: Loadable<SessionStatus>; report: Loadable<ValidationReportResponse>; onRetry: () => void }) {
  const vm = reportVm(report);
  const loading = report.kind === 'loading' || report.kind === 'idle';
  return (
    <section className={screenStyles.screen} aria-labelledby="screen-heading" aria-busy={loading ? 'true' : undefined}>
      <ScreenHeading>About this application</ScreenHeading>
      <p className={styles.headline}>
        <span aria-hidden="true">△ </span>
        {vm.headline}
      </p>
      {vm.failure === null ? null : <FailureAlert failure={vm.failure} onRetry={onRetry} />}

      <h3 className={styles.subheading} id="about-build-heading">
        This build
      </h3>
      {status.kind === 'ready' ? <Entries entries={aboutVm(status.value).entries} /> : <p className={screenStyles.muted}>Build identity is shown here once the session status has been read.</p>}
      <p>
        Every parameter this build computes with, its source, as-of date, vintage, projection and rounding rule, is in the{' '}
        <a className={screenStyles.inlineLink} href={hrefFor({ screen: 'assumptions' }) ?? '#/'}>
          Assumptions Registry
        </a>
        .
      </p>

      <h3 className={styles.subheading} id="about-report-heading">
        Validation report
      </h3>
      <p className={screenStyles.lede}>
        The report is computed from the repository by the build tooling and embedded in this build; sections the design specifies but the tree does not hold yet are listed as not yet introduced with the
        milestone that introduces them, never omitted. Nothing marked pending is counted as validated.
      </p>
      {vm.basis.length === 0 ? null : <Entries entries={vm.basis} />}
      {vm.tables.map((table) => (
        <ReportTable key={table.captionId} table={table} />
      ))}
      {vm.pins.length === 0 ? null : (
        <>
          <h4 className={styles.subheading}>Pins</h4>
          <Entries entries={vm.pins} />
        </>
      )}

      <h3 className={styles.subheading} id="about-scope-heading">
        What is out of scope
      </h3>
      <p className={screenStyles.lede}>
        The threat model this application is built against, and the threats it does not defend against, are stated in the project’s security design (SECURITY.md, section 2). These are the things it does not
        defend against, said plainly:
      </p>
      <dl className={styles.outOfScope}>
        {OUT_OF_SCOPE.map(([threat, told]) => (
          <div key={threat}>
            <dt>{threat}</dt>
            <dd>{told}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
