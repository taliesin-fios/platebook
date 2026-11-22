// SPDX-License-Identifier: LicenseRef-PolyForm-Shield-1.0.0
// Copyright (c) 2026 taliesin-fios. Platebook: https://github.com/taliesin-fios/platebook

/**
 * The first-run tour: its steps, and whether this device has seen it.
 *
 * "Seen" is kept in the browser (localStorage), per account, not on the
 * server: the demo account is shared by every visitor, so a server flag would
 * hide the tour from everyone after the first person closed it.
 */

export interface TourStep {
  icon: string;
  title: string;
  body: string;
}

// Steps describe what each screen is for, not where its button sits, so they
// stay true when the navigation changes.
export const TOUR_STEPS: TourStep[] = [
  {
    icon: '🏠',
    title: 'Dashboard: your week at a glance',
    body: 'See this week’s sessions, what you’ve been paid this month and what’s still owed. The buttons below the numbers start the things you do most: a new session, a new client, a recipe import.',
  },
  {
    icon: '👤',
    title: 'Clients: every household you cook for',
    body: 'Keep each household’s allergies, likes and dislikes, kitchen equipment and parking notes in one place. Platebook warns you when a dish contains something a client can’t eat.',
  },
  {
    icon: '📅',
    title: 'Sessions: plan a cook day',
    body: 'Pick a client, a date and the dishes. Each session builds its own grocery list, grouped by store section, that you tick off on your phone as you shop.',
  },
  {
    icon: '📖',
    title: 'Recipes: your dish library',
    body: 'Add dishes by hand or paste a recipe to import it. Each one carries its ingredients, timing and how well it holds and reheats, ready to drop into a session.',
  },
  {
    icon: '🧾',
    title: 'Invoices: get paid',
    body: 'Turn a finished session into an invoice, email it to the client and mark it paid. Costs shows what you kept after groceries.',
  },
];

type SeenStorage = Pick<Storage, 'getItem' | 'setItem'>;

export const tourSeenKey = (accountId: number) => `platebook.tourSeen.${accountId}`;

// Browsers can refuse storage (private windows, blocked site data): reading
// then counts as "not seen", so the tour shows again instead of breaking.
function browserStorage(): SeenStorage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

export function hasSeenTour(accountId: number, storage: SeenStorage | null = browserStorage()): boolean {
  try {
    return storage?.getItem(tourSeenKey(accountId)) === '1';
  } catch {
    return false;
  }
}

export function markTourSeen(accountId: number, storage: SeenStorage | null = browserStorage()): void {
  try {
    storage?.setItem(tourSeenKey(accountId), '1');
  } catch {
    // Nothing to do: the tour will simply offer itself again next time.
  }
}
