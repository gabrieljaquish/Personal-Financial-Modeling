import styles from './Form.module.css';

export interface SummaryItem {
  readonly inputId: string;
  readonly text: string;
}

/**
 * The error summary of a form. It receives focus on a failed submit and is a
 * named group, NOT a live region: a newly inserted alert that then takes focus is
 * announced twice, so focus alone announces it. Focus on a bare group speaks only
 * its name and role ("There is a problem, group"), so the group is also DESCRIBED BY
 * its intro and its list: a description is spoken with the name when focus lands,
 * which puts the errors themselves in that one announcement without a live region.
 * Whether real screen readers do so is a blocking question of the deferred
 * assistive-technology pass (web/README.md) - settle it before another form copies this.
 * Each entry is a button that moves focus to its field - never an `#id` anchor,
 * which would collide with hash routing. An entry is a claim that THAT field is
 * wrong: the field it focuses must carry `aria-invalid` and its own message. When
 * nobody knows which field is wrong (a server refusal that names none), pass no
 * items: the summary is then the intro alone, with no list and no jump.
 */
export function FormErrorSummary({
  id,
  intro,
  items,
  onJump,
}: {
  id: string;
  intro: string | null;
  items: readonly SummaryItem[];
  onJump: (inputId: string) => void;
}) {
  const titleId = `${id}-title`;
  const introId = `${id}-intro`;
  const listId = `${id}-list`;
  const describedBy = [intro === null ? null : introId, items.length === 0 ? null : listId].filter((part) => part !== null).join(' ');
  return (
    <div className={styles.errorSummary} id={id} role="group" aria-labelledby={titleId} aria-describedby={describedBy === '' ? undefined : describedBy} tabIndex={-1}>
      <h3 className={styles.errorTitle} id={titleId}>
        <span aria-hidden="true">! </span>
        There is a problem
      </h3>
      {intro === null ? null : <p id={introId}>{intro}</p>}
      {items.length === 0 ? null : (
        <ul className={styles.errorList} id={listId} role="list">
          {items.map((item) => (
            <li key={`${item.inputId}:${item.text}`}>
              <button className={styles.errorJump} type="button" onClick={() => onJump(item.inputId)}>
                {item.text}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
