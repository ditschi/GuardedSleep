/**
 * Runtime formatter for the fixed ICU subset produced by tools/i18n-gen:
 * `{arg}`, `{n, plural, =0 {…} one {# …} other {…}}`, `{k, select, a {…} other {…}}`, `''` escaping.
 * The generator validates every catalogue message, so this parser stays lenient and small.
 */

type Case = { sel: string; body: Node[] };
type Node =
  | { t: "text"; v: string }
  | { t: "arg"; name: string }
  | { t: "pound" }
  | { t: "plural" | "select"; name: string; cases: Case[] };

export type MessageParams = Record<string, string | number>;

class ParseError extends Error {}

const parsed = new Map<string, Node[]>();
const pluralRules = new Map<string, Intl.PluralRules>();
const numberFormats = new Map<string, Intl.NumberFormat>();

export function parseMessage(src: string): Node[] {
  let pos = 0;

  const ws = () => {
    while (pos < src.length && /\s/.test(src[pos]!)) pos++;
  };
  const word = () => {
    ws();
    const m = /^[^\s,{}]+/.exec(src.slice(pos));
    if (!m) throw new ParseError(`expected word at ${pos}`);
    pos += m[0].length;
    return m[0];
  };

  function message(inPlural: boolean, top: boolean): Node[] {
    const out: Node[] = [];
    let text = "";
    const flush = () => {
      if (text) out.push({ t: "text", v: text });
      text = "";
    };
    while (pos < src.length) {
      const c = src[pos]!;
      if (c === "}") {
        if (top) throw new ParseError(`unmatched "}" at ${pos}`);
        break;
      }
      if (c === "{") {
        flush();
        out.push(argument(inPlural));
      } else if (c === "#" && inPlural) {
        flush();
        out.push({ t: "pound" });
        pos++;
      } else if (c === "'") {
        const next = src[pos + 1];
        if (next === "'") {
          text += "'";
          pos += 2;
        } else if (next === "{" || next === "}" || (next === "#" && inPlural)) {
          pos++;
          while (pos < src.length) {
            if (src[pos] === "'") {
              if (src[pos + 1] === "'") {
                text += "'";
                pos += 2;
                continue;
              }
              pos++;
              break;
            }
            text += src[pos++];
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
    return out;
  }

  function argument(inPlural: boolean): Node {
    pos++;
    const name = word();
    ws();
    if (src[pos] === "}") {
      pos++;
      return { t: "arg", name };
    }
    if (src[pos] !== ",") throw new ParseError(`expected "," at ${pos}`);
    pos++;
    const kind = word();
    if (kind !== "plural" && kind !== "select") throw new ParseError(`unsupported type "${kind}"`);
    ws();
    if (src[pos] !== ",") throw new ParseError(`expected "," at ${pos}`);
    pos++;
    const cases: Case[] = [];
    for (;;) {
      ws();
      if (src[pos] === "}") {
        pos++;
        break;
      }
      if (pos >= src.length) throw new ParseError("unterminated argument");
      const sel = word();
      ws();
      if (src[pos] !== "{") throw new ParseError(`expected "{" at ${pos}`);
      pos++;
      const body = message(kind === "plural" ? true : inPlural, false);
      if (src[pos] !== "}") throw new ParseError(`unterminated case at ${pos}`);
      pos++;
      cases.push({ sel, body });
    }
    return { t: kind, name, cases };
  }

  return message(false, true);
}

function rules(locale: string): Intl.PluralRules {
  let r = pluralRules.get(locale);
  if (!r) pluralRules.set(locale, (r = new Intl.PluralRules(locale)));
  return r;
}

function formatNumber(n: number, locale: string): string {
  let f = numberFormats.get(locale);
  if (!f) numberFormats.set(locale, (f = new Intl.NumberFormat(locale)));
  return f.format(n);
}

function render(
  nodes: Node[],
  params: MessageParams | undefined,
  locale: string,
  count: number | null,
): string {
  let out = "";
  for (const n of nodes) {
    if (n.t === "text") out += n.v;
    else if (n.t === "arg") {
      const v = params?.[n.name];
      out += v === undefined ? `{${n.name}}` : String(v);
    } else if (n.t === "pound") out += count === null ? "#" : formatNumber(count, locale);
    else if (n.t === "plural") {
      const raw = params?.[n.name];
      const num = Number(raw);
      if (raw === undefined || Number.isNaN(num)) {
        out += `{${n.name}}`;
        continue;
      }
      const chosen =
        n.cases.find((c) => c.sel === `=${num}`) ??
        n.cases.find((c) => c.sel === rules(locale).select(num)) ??
        n.cases.find((c) => c.sel === "other");
      out += chosen ? render(chosen.body, params, locale, num) : "";
    } else {
      const v = String(params?.[n.name] ?? "");
      const chosen = n.cases.find((c) => c.sel === v) ?? n.cases.find((c) => c.sel === "other");
      out += chosen ? render(chosen.body, params, locale, count) : "";
    }
  }
  return out;
}

/** Formats a catalogue message; an unparsable template is returned unchanged. */
export function formatMessage(
  template: string,
  params: MessageParams | undefined,
  locale: string,
): string {
  let ast = parsed.get(template);
  if (!ast) {
    try {
      ast = parseMessage(template);
    } catch {
      return template;
    }
    parsed.set(template, ast);
  }
  return render(ast, params, locale, null);
}
