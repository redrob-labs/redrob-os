// Localized config-schema text (section names, section help, field
// descriptions). The gateway serves this text in English, straight from the
// Rust doc comments; this maps it to the dashboard locale.
//
// Lookup is by the English text with whitespace collapsed, so it does not
// depend on how the schema generator joins doc-comment lines. The key also
// spells the product as Redrob OS and drops em dashes, the same way the
// gateway's own copy does, so an older gateway still finds its entries.
// Text with no entry is shown in English. The Korean table is loaded only
// when needed.

import { getLocale } from './i18n';

type Table = Record<string, string>;

let ko: Table | null = null;
let koLoading: Promise<void> | null = null;

const norm = (s: string) =>
  s
    .replace(/(?<![\w"@:?-])ZeroClaw(?![\w"])/g, 'Redrob OS')
    .replace(/`zeroclaw /g, '`redrob-agentctl ')
    .replace(/\s*\u2014\s*/g, ' - ')
    .replace(/\s+/g, ' ')
    .trim();

/** Load the table for the current locale. Call before using `trSchema`. */
export function loadSchemaText(): Promise<void> {
  if (getLocale() !== 'ko' || ko) return Promise.resolve();
  if (!koLoading) {
    koLoading = import('./schemaText.ko.json')
      .then((m) => {
        ko = m.default as Table;
      })
      .catch(() => {
        koLoading = null;
      });
  }
  return koLoading;
}

/** Schema text in the current locale, or the English text unchanged. */
export function trSchema(text: string): string;
export function trSchema(text: string | undefined): string | undefined;
export function trSchema(text: string | undefined): string | undefined {
  if (!text || getLocale() !== 'ko' || !ko) return text;
  return ko[norm(text)] ?? text;
}
