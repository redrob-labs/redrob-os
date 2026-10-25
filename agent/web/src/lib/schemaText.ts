// Localized server text. The gateway sends this text in English: config
// schema text (section names, section help, field descriptions) straight
// from the Rust doc comments, field names, tool and integration
// descriptions, and doctor messages. This maps it to the dashboard locale.
//
// Lookup is by the English text with whitespace collapsed, so it does not
// depend on how the schema generator joins doc-comment lines. The key also
// spells the product as Redrob OS and drops em dashes, the same way the
// gateway's own copy does, so an older gateway still finds its entries.
// Text with no entry is shown in English, after the same clean-up. The
// Korean tables are loaded only when needed.

import { getLocale } from './i18n';

type Table = Record<string, string>;

interface KoTables {
  text: Table;
  fields: Table;
  doctor: { re: RegExp; ko: string }[];
}

let ko: KoTables | null = null;
let koLoading: Promise<void> | null = null;

const norm = (s: string) =>
  s
    .replace(/(?<![\w"@:?-])ZeroClaw(?![\w"])/g, 'Redrob OS')
    .replace(/`zeroclaw /g, '`redrob-agentctl ')
    .replace(/\s*\u2014\s*/g, ' - ')
    .replace(/\s+/g, ' ')
    .trim();

const escapeRe = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

// A doctor template such as `{0}: model: {1}` becomes `^(.+?): model: (.+?)$`.
function compileDoctor(table: Table): KoTables['doctor'] {
  return Object.entries(table).map(([en, k]) => ({
    re: new RegExp(
      '^' + norm(en).split(/\{\d+\}/).map(escapeRe).join('(.+?)') + '$',
    ),
    ko: k,
  }));
}

/** Load the tables for the current locale. Call before using `trSchema`. */
export function loadSchemaText(): Promise<void> {
  if (getLocale() !== 'ko' || ko) return Promise.resolve();
  if (!koLoading) {
    koLoading = Promise.all([
      import('./schemaText.ko.json'),
      import('./serverText.ko.json'),
      import('./fieldLabels.ko.json'),
    ])
      .then(([schema, server, fields]) => {
        ko = {
          text: { ...(schema.default as Table), ...(server.default.texts as Table) },
          fields: fields.default as Table,
          doctor: compileDoctor(server.default.doctor as Table),
        };
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
  return ko.text[norm(text)] ?? text;
}

/**
 * Other server text (tool and integration descriptions, category labels):
 * the current locale, or the English text with the product name and em
 * dashes cleaned up.
 */
export function trServer(text: string): string {
  if (!text) return text;
  const n = norm(text);
  return (getLocale() === 'ko' && ko?.text[n]) || n;
}

/** A doctor message in the current locale, filling in the template slots. */
export function trDoctor(text: string): string {
  const n = norm(text);
  if (getLocale() !== 'ko' || !ko) return n;
  for (const { re, ko: k } of ko.doctor) {
    const m = n.match(re);
    if (m) return k.replace(/\{(\d+)\}/g, (_, i) => m[Number(i) + 1] ?? '');
  }
  return n;
}

/** Korean label for a config field name, or undefined to use the English. */
export function fieldLabelKo(name: string): string | undefined {
  if (getLocale() !== 'ko' || !ko) return undefined;
  return ko.fields[name];
}
