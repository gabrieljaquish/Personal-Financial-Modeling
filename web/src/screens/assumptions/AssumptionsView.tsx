import { FailureAlert } from '../../components/FailureAlert.tsx';
import { SortableTable } from '../../components/SortableTable.tsx';
import type { Column, SortState } from '../../components/SortableTable.tsx';
import { VerificationBadge } from '../../components/VerificationBadge.tsx';
import tableStyles from '../../components/Table.module.css';
import type { DetailVm, Entry, GridVm, RegistryVm, SeriesRowVm, SourceVm, TableRowVm, VintageVm } from '../../viewmodel/registry.ts';
import type { Loadable } from '../../viewmodel/status.ts';
import { NotFoundScreen } from '../not-found/NotFoundScreen.tsx';
import { ScreenHeading } from '../ScreenHeading.tsx';
import screenStyles from '../Screen.module.css';
import styles from './Assumptions.module.css';

export const DEFAULT_SORT: SortState = { key: 'id', direction: 'ascending' };

function RowLink({ id, href }: { id: string; href: string | null }) {
  return href === null ? (
    <span>{id}</span>
  ) : (
    <a className={tableStyles.rowLink} href={href}>
      {id}
    </a>
  );
}

const TABLE_COLUMNS: readonly Column<TableRowVm>[] = [
  { key: 'id', header: 'Parameter id', sortable: true, rowHeader: true, cell: (row) => <RowLink id={row.id} href={row.href} /> },
  { key: 'period', header: 'Period', secondary: true, cell: (row) => row.period },
  { key: 'asOf', header: 'As of', sortable: true, cell: (row) => row.asOf },
  { key: 'vintage', header: 'Vintage', secondary: true, cell: (row) => `${row.vintageShort} (${row.lockText})` },
  { key: 'verification', header: 'Verification', sortable: true, cell: (row) => <VerificationBadge badge={row.badge} /> },
  { key: 'projection', header: 'Projection', secondary: true, cell: (row) => row.projection },
  { key: 'rounding', header: 'Rounding', secondary: true, cell: (row) => row.rounding },
  { key: 'sources', header: 'Sources', secondary: true, cell: (row) => row.sourceCount },
  { key: 'openItems', header: 'Open items', sortable: true, cell: (row) => row.openItemCount },
];

const SERIES_COLUMNS: readonly Column<SeriesRowVm>[] = [
  { key: 'id', header: 'Series', sortable: true, rowHeader: true, cell: (row) => <RowLink id={row.id} href={row.href} /> },
  { key: 'name', header: 'Name tables use', cell: (row) => row.name },
  { key: 'publisherId', header: 'Publisher’s id', secondary: true, cell: (row) => row.publisherId },
  { key: 'asOf', header: 'As of', sortable: true, cell: (row) => row.asOf },
  { key: 'years', header: 'Years covered', cell: (row) => row.yearsCovered },
  { key: 'verification', header: 'Verification', sortable: true, cell: (row) => <VerificationBadge badge={row.badge} /> },
  { key: 'sources', header: 'Sources', secondary: true, cell: (row) => row.sourceCount },
  { key: 'openItems', header: 'Open items', sortable: true, cell: (row) => row.openItemCount },
];

function Entries({ entries }: { entries: readonly Entry[] }) {
  return (
    <dl className={styles.entries}>
      {entries.map(([label, value]) => (
        <div key={label}>
          <dt>{label}</dt>
          <dd>{value}</dd>
        </div>
      ))}
    </dl>
  );
}

function Grid({ grid, captionId }: { grid: GridVm; captionId: string }) {
  return (
    <div className={tableStyles.scrollRegion} role="region" aria-labelledby={captionId} tabIndex={0}>
      <table className={tableStyles.table}>
        <caption id={captionId}>{grid.caption}</caption>
        <thead>
          <tr>
            <th scope="col">{grid.rowHeader}</th>
            {grid.columns.map((column) => (
              <th key={column} scope="col" className={tableStyles.amountHead}>
                {column}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {grid.rows.map((row) => (
            <tr key={row.header}>
              <th scope="row">{row.header}</th>
              {row.cells.map((cell, index) => (
                <td key={grid.columns[index] ?? `extra-${String(index)}`} className={tableStyles.amount}>
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

function Source({ source }: { source: SourceVm }) {
  return (
    <li className={styles.source}>
      <p className={styles.sourceTitle}>{source.title}</p>
      <dl className={styles.entries}>
        {source.entries.map(([label, value]) => (
          <div key={label}>
            <dt>{label}</dt>
            <dd>{value}</dd>
          </div>
        ))}
        <div>
          {/* Shown, never fetched and never linked: the application makes no outbound request. */}
          <dt>Retrieved from (shown as text; the application does not open it)</dt>
          <dd>
            <code className={styles.code}>{source.url}</code>
          </dd>
        </div>
        <div>
          <dt>SHA-256 of the archived copy</dt>
          <dd>
            <code className={styles.code}>{source.sha256}</code>
          </dd>
        </div>
      </dl>
    </li>
  );
}

/** The words that mark the entry the address names. Words and a border style, never colour alone. */
export const LINKED_ENTRY_MARK = 'Linked entry';

/**
 * `routed`: this is the entry the address (`#/assumptions/<id>`) names. It is
 * identified three ways - `aria-current` on the article, words in its heading (the
 * heading is the focus target of a navigation, so the words are what is spoken),
 * and a border style - so a deep link never renders the same as the index.
 */
function Detail({ detail, routed }: { detail: DetailVm; routed: boolean }) {
  return (
    <article className={routed ? `${styles.detail ?? ''} ${styles.detailRouted ?? ''}` : styles.detail} aria-labelledby={detail.headingId} aria-current={routed ? 'true' : undefined}>
      <h4 className={styles.detailHeading} id={detail.headingId} tabIndex={-1}>
        {`${detail.id} - ${detail.kind}`}
        {routed ? <span className={styles.linkedMark}>{` - ${LINKED_ENTRY_MARK}`}</span> : null}
      </h4>
      <p>
        <VerificationBadge badge={detail.badge} />
      </p>
      <Entries entries={detail.identity} />
      {detail.projection.length === 0 ? null : (
        <>
          <h5 className={styles.subheading}>Projection rule</h5>
          <Entries entries={detail.projection} />
        </>
      )}
      {detail.kind === 'Parameter table' ? (
        <>
          <h5 className={styles.subheading}>Rounding rule</h5>
          {detail.rounding.length === 0 ? <p>No rounding rule is stated for this table.</p> : <Entries entries={detail.rounding} />}
        </>
      ) : null}
      {detail.grids.length === 0 ? null : (
        <>
          <h5 className={styles.subheading}>Values</h5>
          {detail.grids.map((grid, index) => (
            <Grid key={grid.caption} grid={grid} captionId={`${detail.headingId}-grid-${String(index)}`} />
          ))}
        </>
      )}
      <h5 className={styles.subheading}>Sources</h5>
      {detail.sources.length === 0 ? <p>No source is recorded.</p> : <ul className={styles.sources} role="list">{detail.sources.map((source) => <Source key={`${source.title}:${source.sha256}`} source={source} />)}</ul>}
      <h5 className={styles.subheading}>Questions a person still has to answer about this {detail.kind === 'Index series' ? 'series' : 'table'}</h5>
      {detail.openItems.length === 0 ? <p>None recorded.</p> : <ul className={styles.openItems} role="list">{detail.openItems.map((item) => <li key={item}>{item}</li>)}</ul>}
    </article>
  );
}

function Vintage({ vintage, index, routedId, onSorted }: { vintage: VintageVm; index: number; routedId: string | null; onSorted: (message: string) => void }) {
  const prefix = `vintage-${String(index)}`;
  return (
    <section className={styles.vintage} aria-labelledby={`${prefix}-heading`}>
      <h3 className={styles.vintageHeading} id={`${prefix}-heading`}>{`Vintage ${vintage.name}`}</h3>
      <p className={styles.statusLine}>
        <span aria-hidden="true">{vintage.settled ? '● ' : '△ '}</span>
        {vintage.statusLine}
      </p>
      <p className={styles.narrowNote}>Showing the identifying columns only. Every field of an entry is in its detail below.</p>
      <SortableTable
        caption={`Parameter tables of vintage ${vintage.name}`}
        captionId={`${prefix}-tables`}
        columns={TABLE_COLUMNS}
        rows={vintage.tables}
        initialSort={DEFAULT_SORT}
        secondaryClassName={styles.secondaryCol ?? ''}
        onSorted={onSorted}
      />
      <SortableTable
        caption={`Index series of vintage ${vintage.name}`}
        captionId={`${prefix}-series`}
        columns={SERIES_COLUMNS}
        rows={vintage.series}
        initialSort={DEFAULT_SORT}
        secondaryClassName={styles.secondaryCol ?? ''}
        onSorted={onSorted}
      />
      {vintage.details.map((detail) => (
        <Detail key={detail.id} detail={detail} routed={detail.id === routedId} />
      ))}
    </section>
  );
}

/** The registry as a pure function of what was loaded and which entry is routed to. */
export function AssumptionsView({
  registry,
  routedId,
  onRetry,
  onSorted,
}: {
  registry: Loadable<RegistryVm>;
  routedId: string | null;
  onRetry: () => void;
  onSorted: (message: string) => void;
}) {
  if (registry.kind === 'ready' && routedId !== null && !registry.value.ids.includes(routedId)) {
    return <NotFoundScreen />;
  }
  return (
    <section className={screenStyles.screen} aria-labelledby="screen-heading" aria-busy={registry.kind === 'loading' ? 'true' : undefined}>
      <ScreenHeading>Assumptions Registry</ScreenHeading>
      <p className={screenStyles.lede}>
        Every parameter this build computes with: its source, as-of date, vintage, projection rule and rounding rule, and the state of its verification by a person.
      </p>
      {registry.kind === 'idle' || registry.kind === 'loading' ? <p className={screenStyles.muted}>Reading the registry…</p> : null}
      {registry.kind === 'failed' ? <FailureAlert failure={registry.failure} onRetry={onRetry} /> : null}
      {registry.kind === 'ready' && registry.value.vintages.length === 0 ? <p>This build carries no parameter vintage.</p> : null}
      {registry.kind === 'ready' ? registry.value.vintages.map((vintage, index) => <Vintage key={vintage.name} vintage={vintage} index={index} routedId={routedId} onSorted={onSorted} />) : null}
    </section>
  );
}
