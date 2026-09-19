import type { ExportFormatDto } from '../api/schema.gen.ts';
import styles from './Form.module.css';

const WARNING_ID = 'export-warning';

/** The plain warning. It names BOTH ways out - a download and a print - because it stands in front of both. */
export const EXPORT_WARNING =
  'A downloaded file or a printed page contains the taxable income you entered, and leaves the application’s protection. A download is saved where your browser saves downloads; a printed page or a PDF goes wherever you send it.';

/**
 * Download and print: the one place plaintext leaves the application
 * (ARCHITECTURE.md §5, SECURITY.md §9 - "user-initiated, preceded by a plain
 * warning"). The warning comes first in reading and focus order, ahead of all
 * three buttons, and every button is described by it.
 */
export function ExportActions({ busy, onExport, onPrint }: { busy: boolean; onExport: (format: ExportFormatDto) => void; onPrint: () => void }) {
  return (
    <div className={styles.actions}>
      <p className={styles.hint} id={WARNING_ID}>
        {EXPORT_WARNING}
      </p>
      <div className={styles.actionRow}>
        {/* `aria-disabled`, not `disabled`: a disabled button drops focus. Clicks are ignored while busy. */}
        <button type="button" aria-describedby={WARNING_ID} aria-disabled={busy ? 'true' : undefined} onClick={() => onExport('csv')}>
          Download CSV
        </button>
        <button type="button" aria-describedby={WARNING_ID} aria-disabled={busy ? 'true' : undefined} onClick={() => onExport('json')}>
          Download JSON
        </button>
        <button type="button" aria-describedby={WARNING_ID} onClick={onPrint}>
          Print
        </button>
      </div>
    </div>
  );
}
