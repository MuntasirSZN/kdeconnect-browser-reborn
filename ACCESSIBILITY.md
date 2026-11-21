# Accessibility Statement

_A KDE Connect Browser Reborn project._ Based on the
[accessibility best practices for your project](https://opensource.guide/accessibility-best-practices-for-your-project/#start-with-an-accessibility-statement).

## What is this project?

[KDE Connect Browser Reborn](https://github.com/MuntasirSZN/kdeconnect-browser-reborn)
is a Firefox and Chromium-based extension that sends pages and content from
your browser to connected KDE Connect devices — via the browser action
popup, or the right-click context menu. It supports both Firefox/Zen and
Chrome (MV3) in parallel.

## Status of accessibility

The project is in the **growing** stage: the browser UI is small (a device
list popup plus native context menus), and we are building it up steadily.

What we do well today:

- **Text-based UI.** All information is rendered as real text
  (`textContent`). Device names are never converted to images or ASCII art.
- **Semantic markup.** The popup and options pages wrap their content in a
  `<main>` landmark; popup rows are `<button>` elements; icon glyphs are
  `<i>` elements carrying `data-lucide` + `aria-hidden="true"` (they are
  decorative, their meaning is conveyed by adjacent text); every action
  has a visible text label (e.g. "Send").
- **Clear colour contrast.** The UI follows the system colour scheme
  (`prefers-color-scheme`), keeps dark mode high-contrast, and does not rely
  on colour alone to convey meaning. WCAG-verified: the Save button uses dark
  text on KDE Connect orange (5.46:1); option page links use
  `#8a3b00` on `#fff` (7.76:1); the popup's hover state is `#fd8833` at
  6.63:1.
- **Focus rings.** `:focus-visible` outlines 2px solid accent (popup:
  offset 1px, options: offset 2px).
- **Decorative status.** Online dots use `--success` (#2ecc71 in the popup;
  options: #2e7d32 light / #66bb6a dark); the status dot is decorative
  (`aria-hidden="true"`); every action has a text label (e.g. "Send").
- **Live status messages.** The popup's status bar uses `aria-live="polite"`.
- **Loading and empty states.** "Loading…" / "No devices found…" render a
  Lucide `Smartphone` icon + text via `renderEmpty()`.

axe-core 4.12.1 audit of the built `dist/popup.html` and `dist/options.html`:
**0 violations**.

What we plan to do next:

- Keep the popup keyboard-friendly (Chrome/Firefox handle tab focus for
  native menus already).
- Audit the UI with [axe-core](https://github.com/dequelabs/axe-core)
  via `agent-browser a11y audit` and fix findings.
- Expand the accessibility statement as the project grows.

## We welcome feedback

If you encounter an accessibility issue in this project, or have suggestions
for improvement, please open an issue or contact us:

- GitHub issues: https://github.com/MuntasirSZN/kdeconnect-browser-reborn/issues
- Email: muntasir.joypurhat@gmail.com

## Disclaimer

This statement is not an A11Y guarantee. We welcome feedback from users and
contributors alike.
