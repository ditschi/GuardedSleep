/**
 * Runtime formatter for the fixed ICU subset that `crates/i18n-gen` validates: `{arg}`,
 * `{n, plural, =0 {...} one {# ...} other {...}}`, `{k, select, a {...} other {...}}`, `#` inside
 * a plural and apostrophe quoting. The generator rejects anything else at build time, so this
 * parser stays small; an unparsable template is shown as written rather than throwing in the UI.
 */

type Case = { selector: string; body: Node[] };
type Node =
  | { kind: "text"; value: string }
  | { kind: "arg"; name: string }
  | { kind: "pound" }
  | { kind: "plural" | "select"; name: string; cases: Case[] };

export type MessageArgs = Readonly<Record<string, string | number>>;

class ParseError extends Error {}

function parseMessage(src: string): Node[] {
  let pos = 0;

  const skipSpace = (): void => {
    while (pos < src.length && /\s/.test(src.charAt(pos))) pos++;
  };

  const word = (): string => {
    skipSpace();
    const match = /^[^\s,{}]+/.exec(src.slice(pos));
    if (!match) throw new ParseError(`expected a word at ${pos}`);
    pos += match[0].length;
    return match[0];
  };

  const expect = (char: string): void => {
    skipSpace();
    if (src.charAt(pos) !== char) throw new ParseError(`expected "${char}" at ${pos}`);
    pos++;
  };

  function message(inPlural: boolean, top: boolean): Node[] {
    const nodes: Node[] = [];
    let text = "";
    const flush = (): void => {
      if (text) nodes.push({ kind: "text", value: text });
      text = "";
    };
    while (pos < src.length) {
      const c = src.charAt(pos);
      if (c === "}") {
        if (top) throw new ParseError(`unmatched "}" at ${pos}`);
        break;
      }
      if (c === "{") {
        flush();
        nodes.push(argument(inPlural));
      } else if (c === "#" && inPlural) {
        flush();
        nodes.push({ kind: "pound" });
        pos++;
      } else if (c === "'") {
        const next = src.charAt(pos + 1);
        if (next === "'") {
          text += "'";
          pos += 2;
        } else if (next === "{" || next === "}" || (next === "#" && inPlural)) {
          // Quoted literal run: everything up to the closing apostrophe is plain text.
          pos++;
          while (pos < src.length) {
            if (src.charAt(pos) === "'") {
              if (src.charAt(pos + 1) === "'") {
                text += "'";
                pos += 2;
                continue;
              }
              pos++;
              break;
            }
            text += src.charAt(pos++);
          }
        } else {
          text += "'";
          pos++;
        }
      } else {
        text += c;
        pos++;
      }
    }
    flush();
    return nodes;
  }

  function argument(inPlural: boolean): Node {
    expect("{");
    const name = word();
    skipSpace();
    if (src.charAt(pos) === "}") {
      pos++;
      return { kind: "arg", name };
    }
    expect(",");
    const kind = word();
    if (kind !== "plural" && kind !== "select") {
      throw new ParseError(`unsupported argument type "${kind}"`);
    }
    expect(",");
    const cases: Case[] = [];
    for (;;) {
      skipSpace();
      if (src.charAt(pos) === "}") {
        pos++;
        break;
      }
      if (pos >= src.length) throw new ParseError("unterminated argument");
      const selector = word();
      expect("{");
      const body = message(kind === "plural" ? true : inPlural, false);
      expect("}");
      cases.push({ selector, body });
    }
    return { kind, name, cases };
  }

  return message(false, true);
}

const parsed = new Map<string, Node[] | null>();
const pluralRules = new Map<string, Intl.PluralRules>();
const numberFormats = new Map<string, Intl.NumberFormat>();

function cached<V>(cache: Map<string, V>, key: string, make: () => V): V {
  let value = cache.get(key);
  if (value === undefined) {
    value = make();
    cache.set(key, value);
  }
  return value;
}

function render(
  nodes: readonly Node[],
  args: MessageArgs | undefined,
  locale: string,
  count: number | null,
): string {
  let out = "";
  for (const node of nodes) {
    switch (node.kind) {
      case "text":
        out += node.value;
        break;
      case "arg": {
        const value = args?.[node.name];
        out += value === undefined ? `{${node.name}}` : String(value);
        break;
      }
      case "pound":
        out +=
          count === null
            ? "#"
            : cached(numberFormats, locale, () => new Intl.NumberFormat(locale)).format(count);
        break;
      case "plural": {
        const raw = args?.[node.name];
        const n = Number(raw);
        if (raw === undefined || Number.isNaN(n)) {
          out += `{${node.name}}`;
          break;
        }
        const category = cached(pluralRules, locale, () => new Intl.PluralRules(locale)).select(n);
        const chosen =
          node.cases.find((c) => c.selector === `=${n}`) ??
          node.cases.find((c) => c.selector === category) ??
          node.cases.find((c) => c.selector === "other");
        out += chosen ? render(chosen.body, args, locale, n) : "";
        break;
      }
      case "select": {
        const value = String(args?.[node.name] ?? "");
        const chosen =
          node.cases.find((c) => c.selector === value) ??
          node.cases.find((c) => c.selector === "other");
        out += chosen ? render(chosen.body, args, locale, count) : "";
        break;
      }
    }
  }
  return out;
}

/** Formats one catalogue message for a locale; an unparsable template is returned unchanged. */
export function formatMessage(
  template: string,
  args: MessageArgs | undefined,
  locale: string,
): string {
  const ast = cached(parsed, template, () => {
    try {
      return parseMessage(template);
    } catch {
      return null;
    }
  });
  return ast === null ? template : render(ast, args, locale, null);
}
