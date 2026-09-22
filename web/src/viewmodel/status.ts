// Verification status, shown honestly. Every mapping here fails closed: an
// unknown value is "treat as unverified", the word "verified" is produced only
// for `verified === true`, the word "locked" only when a locked id is present,
// and the two facts are reported separately - never merged into one reassuring
// word. Lookups only.

import type { ApiFailure } from '../api/errors.ts';
import type { SessionStatus } from '../api/schema.gen.ts';

export type BadgeTone = 'attention' | 'confirmed';

export interface BadgeVm {
  readonly text: string;
  /** Chooses the decorative glyph and border style. The text carries the meaning. */
  readonly tone: BadgeTone;
  /** Pending first: what needs a person's attention leads a sorted table. */
  readonly rank: number;
}

const BADGES: Readonly<Record<string, BadgeVm>> = {
  'pending-hand-verification': { text: 'Pending hand verification - not for decisions', tone: 'attention', rank: 0 },
  unstated: { text: 'Verification not stated - treat as unverified', tone: 'attention', rank: 1 },
  'hand-worked-reviewed': { text: 'Hand-worked and reviewed', tone: 'confirmed', rank: 3 },
  'primary-source-confirmed': { text: 'Confirmed against the primary source', tone: 'confirmed', rank: 4 },
};

const UNKNOWN_BADGE: BadgeVm = { text: 'Unknown status - treat as unverified', tone: 'attention', rank: 2 };

export function badgeFor(verification: unknown): BadgeVm {
  return typeof verification === 'string' && Object.hasOwn(BADGES, verification) ? (BADGES[verification] ?? UNKNOWN_BADGE) : UNKNOWN_BADGE;
}

export interface VintageIdentity {
  readonly name: string;
  readonly verified?: unknown;
  readonly lockedId?: string | null | undefined;
}

export function isLocked(vintage: VintageIdentity): boolean {
  return typeof vintage.lockedId === 'string' && vintage.lockedId !== '';
}

/** `true` only for the boolean `true`; anything else is unverified. */
export function isVerified(vintage: VintageIdentity): boolean {
  return vintage.verified === true;
}

/** Whether a vintage's figures may be presented without the warning. */
export function isSettled(vintage: VintageIdentity): boolean {
  return isVerified(vintage) && isLocked(vintage);
}

export function vintageStatusSentence(vintage: VintageIdentity): string {
  const lock = isLocked(vintage) ? 'locked' : 'unlocked';
  const verification = isVerified(vintage) ? 'verified' : 'pending human verification';
  const consequence = isSettled(vintage) ? '' : ' Figures computed from it are not for decisions.';
  return `Parameter vintage ${vintage.name} is ${lock} and ${verification}.${consequence}`;
}

/** What the result of a computation says about the vintage it used. */
export function unverifiedNotice(contentId: string, vintage: VintageIdentity): string {
  const lock = isLocked(vintage) ? 'locked' : 'unlocked';
  const verification = isVerified(vintage) ? 'verified' : 'pending human verification';
  return `Computed from parameter vintage ${contentId}, which is ${lock} and ${verification}. Not for decisions.`;
}

export type Loadable<T> = { kind: 'idle' } | { kind: 'loading' } | { kind: 'failed'; failure: ApiFailure } | { kind: 'ready'; value: T };

export const STATUS_UNREAD = 'Verification status of this build’s parameters could not be read - treat every figure as unverified.';
export const STATUS_READING = 'The verification status of this build’s parameters has not been read yet - until it is, treat every figure as unverified.';

/**
 * The sentences of the persistent banner. It never disappears on an error, and
 * it is empty only when every vintage is both verified and locked.
 */
export function bannerSentences(status: Loadable<SessionStatus>): readonly string[] {
  if (status.kind === 'failed') {
    return [STATUS_UNREAD];
  }
  if (status.kind !== 'ready') {
    return [STATUS_READING];
  }
  if (status.value.vintages.length === 0) {
    return [STATUS_UNREAD];
  }
  return status.value.vintages.filter((vintage) => !isSettled(vintage)).map(vintageStatusSentence);
}

export interface AboutVm {
  readonly entries: readonly (readonly [string, string])[];
}

const TRUST_MODES: Readonly<Record<string, string>> = {
  installed: 'The local certificate authority is installed for this user',
  declined: 'Not installed; the browser warns and the certificate fingerprint is compared by hand',
};

export function aboutVm(status: SessionStatus): AboutVm {
  const trust = Object.hasOwn(TRUST_MODES, status.trustMode) ? (TRUST_MODES[status.trustMode] ?? status.trustMode) : status.trustMode;
  return {
    entries: [
      ['Application version', status.appVersion],
      ['Licence', status.licence],
      ['API version', status.apiVersion],
      ['Certificate trust', trust],
      ...status.vintages.map((vintage): readonly [string, string] => {
        const lock = isLocked(vintage) ? 'locked as ' + (vintage.lockedId ?? '') : 'unlocked';
        return [`Parameter vintage ${vintage.name}`, `${vintage.contentId} (${lock})`];
      }),
    ],
  };
}
