// NIP-11 relay information document (https://github.com/nostr-protocol/nips/blob/master/11.md).
//   node scripts/spike/nip11.mjs [--data data] [--only host,host]
// Prints one JSON object per relay of data/nostr-relays.json. Used by relays-check.mjs, too.
import { pathToFileURL } from "node:url";
import { parseArgs, readData } from "./lib.mjs";

/** Fetches the document of `host`; never throws, `error` names the failure class. */
export async function fetchNip11(host, timeoutMs = 8_000) {
  try {
    const response = await fetch(`https://${host}/`, {
      headers: { Accept: "application/nostr+json" },
      signal: AbortSignal.timeout(timeoutMs),
      redirect: "follow",
    });
    if (!response.ok) return { host, error: `http ${response.status}` };
    const doc = await response.json();
    const limit = doc.limitation ?? {};
    return {
      host,
      name: doc.name ?? null,
      software: doc.software ?? null,
      version: doc.version ?? null,
      supported_nips: doc.supported_nips ?? null,
      limitation: {
        max_message_length: limit.max_message_length ?? null,
        max_content_length: limit.max_content_length ?? null,
        max_event_tags: limit.max_event_tags ?? null,
        created_at_lower_limit: limit.created_at_lower_limit ?? null,
        created_at_upper_limit: limit.created_at_upper_limit ?? null,
        payment_required: limit.payment_required ?? false,
        auth_required: limit.auth_required ?? false,
        restricted_writes: limit.restricted_writes ?? false,
        min_pow_difficulty: limit.min_pow_difficulty ?? 0,
      },
      has_privacy_policy: Boolean(doc.privacy_policy),
    };
  } catch (e) {
    return { host, error: e?.name === "TimeoutError" ? "timeout" : "unreachable" };
  }
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  const args = parseArgs(process.argv.slice(2), {
    data: { default: "data" },
    only: { default: "" },
  });
  const only = args.only ? args.only.split(",") : null;
  const relays = readData(args.data, "nostr-relays.json").relays.filter(
    (r) => !only || only.includes(r.host),
  );
  const docs = await Promise.all(relays.map((r) => fetchNip11(r.host)));
  for (const doc of docs) console.log(JSON.stringify(doc));
}
