# Accessibility Statement

_A KDE Connect Browser Reborn project._ Based on the
[accessibility best practices for your project](https://opensource.guide/accessibility-best-practices-for-your-project/#start-with-an-accessibility-statement).

## What is this project?

[ KDE Connect Browser Reborn](https://github.com/MuntasirSZN/kdeconnect-chrome-extension)
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
- **Semantic markup.** Buttons are `<button>` elements; icon glyphs are
  `<i>` elements carrying `data-lucide` + `aria-hidden="true"` (they are
  decorative, their meaning is conveyed by adjacent text).
- **Clear colour contrast.** The UI follows the system colour scheme
  (`prefers-color-scheme`), keeps dark mode high-contrast, and does not rely
  on colour alone to convey meaning.
- **No icon-only text.** The status dot is decorative
  (`aria-hidden="true"`); every action has a text label (`Send`).

What we plan to do next:

- Keep the popup keyboard-friendly (Chrome/Firefox handle tab focus for
  native menus already).
- Audit the UI with [axe-core](https://github.com/dequelabs/axe-core)
  via `agent-browser a11y audit` and fix findings.
- Expand the accessibility statement as the project grows.

## We welcome feedback

If you encounter an accessibility issue in this project, or have suggestions
for improvement, please open an issue or contact us:

- GitHub issues: https://github.com/MuntasirSZN/kdeconnect-chrome-extension/issues
- Email: muntasir.joypurhat@gmail.com

## Disclaimer

This statement is not an A11Y guarantee. We welcome feedback from users and
contributors alike.
