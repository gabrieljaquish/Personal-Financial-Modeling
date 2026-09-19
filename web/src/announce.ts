// "One message, one channel." Every reducer state declares what it announces as
// data, so the rule is testable without a screen reader:
//
//   * `polite` goes to the one `role="status"` region in the shell;
//   * `alert` is rendered by the view in a `role="alert"` block that never takes focus;
//   * `focus` names a container that receives programmatic focus - and a focused
//     container is never a live region, because a newly inserted alert that then
//     takes focus is announced twice.
//
// For every state, at most one of `polite` and `alert` is set.

export interface Announcements {
  readonly polite?: string;
  readonly alert?: string;
  readonly focus?: string;
}

/**
 * What a screen writes to the shell's polite region for a state. The write is
 * AUTHORITATIVE: a state that declares no `polite` text writes the empty string, so
 * a message never outlives the state that declared it ("Calculating…" left above
 * the alert that says the request failed; one screen's message showing on the next).
 */
export function politeText(announcements: Announcements): string {
  return announcements.polite ?? '';
}

/** A polite message and the screen that wrote it. */
export interface PoliteMessage {
  readonly owner: string;
  readonly text: string;
}

/**
 * What the shell's polite region shows. A message belongs to the screen that wrote
 * it and is not shown on any other ("Sorted by ..." from the registry must not sit
 * above the rate-schedule form); and a lost session empties the region, because its
 * sentence is the session notice's alert.
 */
export function visiblePolite(message: PoliteMessage, owner: string, connected: boolean): string {
  return connected && message.owner === owner ? message.text : '';
}

/** The registry list and a routed registry entry are one screen; every other route is its own. */
export function politeOwner(screen: string): string {
  return screen === 'assumption' ? 'assumptions' : screen;
}

/**
 * The store behind the polite region, as a reducer. Two rules close the stale
 * message (WCAG 4.1.3): a `write` is authoritative, and a `route` event to another
 * owner EMPTIES the store - so a message cannot survive its screen being left, even
 * when the screen left for writes nothing itself (the not-found route) or the
 * screen returned to only ever writes on an action (the registry's "Sorted by ...").
 * A `route` event to the current owner keeps the message: the screen that has just
 * mounted may already have written its own.
 */
export type PoliteEvent = { type: 'write'; owner: string; text: string } | { type: 'route'; owner: string };

export function politeReducer(message: PoliteMessage, event: PoliteEvent): PoliteMessage {
  if (event.type === 'write') {
    return { owner: event.owner, text: event.text };
  }
  return message.owner === event.owner ? message : { owner: event.owner, text: '' };
}
