// Icons on buttons, menus and navigation, picked by what they say, so every "Get latest" in the
// app has the same icon and a new button with a known label needs nothing extra. The icon is
// drawn by CSS (styles/icons.css) from a data-ic attribute, which React leaves alone.
import type { IconName } from './icons';

const LABELS: Array<[string, IconName]> = [
  // navigation
  ['All projects', 'arrow-left'], ['Today', 'calendar-clock'], ['Files', 'files'], ['History', 'history'], ['People', 'users'],
  ['Drafts', 'file-clock'], ['Branches', 'git-branch'], ['Overview', 'layout-dashboard'], ['Invites', 'mail-plus'],
  ['Organizations', 'building-2'], ['Projects', 'folders'], ['Fixing', 'wrench'], ['Audit log', 'scroll-text'], ['Admin', 'shield'], ['Policies', 'shield-check'], ['New policy', 'plus'], ['People…', 'users'], ['Who can see', 'eye'], ['Add rule', 'plus'],
  ['Look', 'palette'], ['Access tokens', 'key-round'], ['Make token', 'key-round'], ['Copy token', 'copy'], ['Get the desktop app', 'download'],
  // work
  ['Get latest', 'cloud-download'], ['Publish', 'cloud-upload'], ['Save draft', 'save'], ['Save checkpoint', 'bookmark-plus'],
  ['Claim', 'lock'], ['Release', 'lock-open'], ['Ask ', 'hand'], ['Download', 'download'], ['Preview', 'eye'], ['Older versions', 'history'],
  ['Put back', 'rotate-ccw'], ['Make latest', 'check-check'], ['Restore', 'rotate-ccw'], ['Add tag', 'tag'], ['Make tag', 'tag'], ['New tag', 'tag'],
  ['New branch', 'git-branch'], ['Make branch', 'git-branch'], ['Merge', 'git-merge'], ['Archive', 'archive'], ['Pick up', 'download'],
  ['Open on the web', 'external-link'], ['Open', 'external-link'], ['See its changes', 'history'], ['Back to Today', 'arrow-left'],
  // admin
  ['New project', 'plus'], ['New organization', 'plus'], ['Make organization', 'building-2'], ['Make project', 'plus'],
  ['Invite people', 'user-plus'], ['Make invites', 'mail-plus'], ['Make invite link', 'link'], ['Copy link', 'copy'], ['Copy', 'copy'],
  ['Revoke', 'x'], ['Remove', 'x'], ['Delete', 'trash-2'], ['Rename', 'pencil'], ['Change storage', 'hard-drive'], ['Export', 'archive'], ['Start export', 'download'], ['Move file data', 'hard-drive'],
  ['Suspend', 'lock'], ['Reactivate', 'lock-open'], ['Set a new password', 'key-round'], ['Set password', 'key-round'],
  ['Sign out everywhere', 'log-out'], ['Sign out', 'log-out'], ['Sign in', 'log-in'], ['Create account and join', 'user-plus'], ['Join ', 'user-plus'],
  ['Back up now', 'save'], ['Send me a test email', 'mail'], ['Send a test', 'send'], ['Add webhook', 'plus'], ['Pause', 'clock'], ['Resume', 'refresh-cw'],
  ['Check storage', 'search'], ['See how much can be reclaimed', 'hard-drive'], ['Reclaim', 'trash-2'], ['Release selected', 'lock-open'],
  ['Review stuck claims', 'lock'], ['See suspended people', 'users'], ['Download as CSV', 'download'], ['Add', 'plus'], ['Try again', 'refresh-cw'],
  ['Go to your projects', 'arrow-right'], ['Go to sign in', 'log-in'],
];

export function iconFor(text: string): IconName | null {
  const t = text.replace(/\s+/g, ' ').trim();
  for (const [label, name] of LABELS) {
    if (label.endsWith(' ') ? t.startsWith(label) : t === label || t.startsWith(label + ' ') || t.startsWith(label + '…') || t.startsWith(label + '(')) return name;
  }
  return null;
}

const SELECTOR = 'button.btn, a.btn, nav a, .nav a, .seg button, .menu button, [role=menuitem]';

function paint(root: ParentNode) {
  root.querySelectorAll<HTMLElement>(SELECTOR).forEach((el) => {
    if (el.dataset.noicon !== undefined) return;
    // Only the element's own words: not a count badge (<em>) beside them.
    const words = [...el.childNodes].filter((n) => n.nodeType === Node.TEXT_NODE || (n as HTMLElement).tagName === 'SPAN').map((n) => n.textContent).join(' ') || el.textContent || '';
    if (el.dataset.icFor === words) return;
    el.dataset.icFor = words;
    const name = iconFor(words);
    if (name) el.dataset.ic = name;
    else delete el.dataset.ic;
  });
}

/** Start once: keeps every button and link labelled with an icon as the page changes. */
export function startAutoIcons() {
  paint(document);
  let queued = 0;
  new MutationObserver(() => {
    if (queued) return;
    queued = requestAnimationFrame(() => { queued = 0; paint(document); });
  }).observe(document.body, { childList: true, subtree: true, characterData: true });
}
