import styles from './App.module.css';

/**
 * The application shell: landmarks, a skip link, labelled regions and one live
 * region, and nothing else. There is no routing (hash routing arrives with the
 * first second screen) and no API call (the session bootstrap and the
 * rate-schedule screen are separate work).
 *
 * The accessibility patterns are established here on purpose: PLAN.md §4.1 puts
 * labelled controls, deliberate focus order, live regions and non-colour
 * encoding at M0 "where they are nearly free", and §4.13 gate item 10 requires
 * `axe` clean at serious and critical on every screen from here on.
 *
 * Styling rule for every component added below this one: class names from a CSS
 * module, never a `style={{ ... }}` prop. The Content-Security-Policy is
 * `style-src 'self'` with no `'unsafe-inline'` (SECURITY.md §7.1), which blocks
 * inline style attributes as well as inline <style> elements, and M0 acceptance
 * is a CSP violation count of zero.
 */
export function App() {
  return (
    <div className={styles.shell}>
      <header className={styles.banner}>
        {/*
          First focusable element in the document, and inside the banner landmark
          so that no visible content sits outside a landmark. It is moved
          off-screen rather than hidden with `display: none`, which would take it
          out of the focus order entirely.
        */}
        <a className={styles.skipLink} href="#main-content">
          Skip to main content
        </a>
        <h1 className={styles.wordmark}>Personal Financial Modeling</h1>
        <p className={styles.tagline}>
          Planning, computed locally. Nothing leaves this machine.
        </p>
      </header>

      <main className={styles.main} id="main-content" tabIndex={-1}>
        {/*
          The single shared live region. Long-running work streams progress over
          NDJSON in a later milestone; announcing it through one polite region
          established now is what keeps that streaming accessible later. It is
          empty at rest, so nothing is announced on load.
        */}
        <p className={styles.liveRegion} role="status" aria-live="polite" />

        <section className={styles.section} aria-labelledby="plan-heading">
          <h2 className={styles.sectionHeading} id="plan-heading">
            Plan
          </h2>
          <p className={styles.placeholder}>
            No plan is open. Opening, unlocking and editing a plan file arrive
            with the encrypted plan container.
          </p>
        </section>

        <section className={styles.section} aria-labelledby="results-heading">
          <h2 className={styles.sectionHeading} id="results-heading">
            Results
          </h2>
          <p className={styles.placeholder}>
            Ledgers, tax lines and projections render here. Every figure will
            carry the explanation and the parameter sources behind it.
          </p>
        </section>

        <section className={styles.section} aria-labelledby="assumptions-heading">
          <h2 className={styles.sectionHeading} id="assumptions-heading">
            Assumptions
          </h2>
          <p className={styles.placeholder}>
            The assumptions registry lists every parameter with its source,
            as-of date, vintage, projection rule and rounding rule.
          </p>
        </section>
      </main>

      <aside className={styles.aside} aria-labelledby="about-heading">
        <h2 className={styles.sectionHeading} id="about-heading">
          About this build
        </h2>
        <p className={styles.placeholder}>
          Build identity, parameter vintages and the validation report are
          reported here once the server supplies them.
        </p>
      </aside>

      <footer className={styles.contentinfo}>
        <p className={styles.footerText}>
          Personal Financial Modeling is planning software, not financial,
          investment, tax or legal advice.
        </p>
      </footer>
    </div>
  );
}
