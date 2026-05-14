<script lang="ts">
  import { browser } from "$app/environment";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import InviteScanner from "$lib/components/InviteScanner.svelte";
  import {
    isCompleteInvite,
    parseInviteFromString,
    parseInviteSearchParams,
    type ParsedInvite,
  } from "$lib/invite-url.js";
  import { buildCompactInviteJson, generatePskBase64Url } from "$lib/pairing/compact-invite.js";
  import {
    getStoredPairingPskB64ForRoom,
    storePairingMaterial,
  } from "$lib/pairing/session-material.js";
  import {
    getEffectiveSignalingUrl,
    isLoopbackSignalingHost,
    validateSignalingUrl,
  } from "$lib/signaling-url.js";
  import { generatePassphraseRoomId, normalizeRoomId } from "$lib/utils/room.js";
  import { tWithEnFallback } from "$lib/i18n/fallback-t.js";
  import QRCode from "qrcode";
  let roomInput = $state("");
  let invitePsk = $state("");
  let error = $state<string | undefined>(undefined);
  let qrDataUrl = $state("");
  /** Parent: paste full invite link, QR JSON, or room code only. */
  let parentJoinDraft = $state("");
  let parentInviteOpen = $state(false);
  /** Avoids duplicate `goto` when the auto-redirect effect runs more than once. */
  let urlAutoNavScheduled = $state(false);

  const role = $derived.by(() => {
    const r = page.url.searchParams.get("role");
    return r === "baby" || r === "parent" ? r : undefined;
  });

  const inviteFromUrl = $derived(parseInviteSearchParams(page.url.searchParams));

  /** Prefer invite signal from URL (session) so QR matches the active stack; else default from settings/build. */
  const signalingForHostQr = $derived.by(() => {
    const fromInvite = inviteFromUrl.signal?.trim();
    if (fromInvite && validateSignalingUrl(fromInvite) === null) return fromInvite;
    return getEffectiveSignalingUrl();
  });

  const showHostQr = $derived(role === "baby" || (role === "parent" && parentInviteOpen));

  const loopbackSignalingForQr = $derived(
    showHostQr && isLoopbackSignalingHost(signalingForHostQr),
  );

  /** Parent already has room+signal in URL — room-code field is only for manual entry. */
  const parentHasInviteInUrl = $derived(role === "parent" && isCompleteInvite(inviteFromUrl));

  $effect(() => {
    if (!browser) return;
    if (inviteFromUrl.room) roomInput = inviteFromUrl.room;
  });

  $effect(() => {
    if (!browser || role !== "baby") return;
    if (inviteFromUrl.room) return;
    if (roomInput.trim() !== "") return;
    roomInput = generatePassphraseRoomId();
  });

  $effect(() => {
    if (!browser || role !== "baby") return;
    if (invitePsk === "") invitePsk = generatePskBase64Url();
  });

  $effect(() => {
    if (!browser || role !== "parent" || !parentInviteOpen) return;
    if (roomInput.trim() === "") {
      roomInput = generatePassphraseRoomId();
    }
    const id = normalizeRoomId(roomInput.trim());
    if (invitePsk === "" && (!id || !getStoredPairingPskB64ForRoom(id))) {
      invitePsk = generatePskBase64Url();
    }
  });

  $effect(() => {
    if (!browser || !showHostQr) {
      qrDataUrl = "";
      return;
    }
    const room = normalizeRoomId(roomInput.trim());
    const sig = signalingForHostQr;
    const k = invitePsk || (room ? getStoredPairingPskB64ForRoom(room) : "") || "";
    if (!room || validateSignalingUrl(sig) !== null || k === "") {
      qrDataUrl = "";
      return;
    }
    const payload = buildCompactInviteJson(room, sig, k);
    let cancelled = false;
    void QRCode.toDataURL(payload, {
      margin: 2,
      width: 280,
      color: { dark: "#0f172a", light: "#ecfdf5" },
    }).then((url) => {
      if (!cancelled) qrDataUrl = url;
    });
    return () => {
      cancelled = true;
    };
  });

  /** When set (e.g. from baby unit "Show pairing QR"), stay on /pair to display the host QR instead of redirecting to /baby. */
  const skipInviteAutoSession = $derived(page.url.searchParams.get("pairHost") === "1");

  /** Complete invite in URL → jump straight into baby or parent session. */
  $effect(() => {
    if (!browser || !role || urlAutoNavScheduled || skipInviteAutoSession) return;
    if (!isCompleteInvite(inviteFromUrl)) return;
    urlAutoNavScheduled = true;
    const q = new URLSearchParams();
    q.set("room", inviteFromUrl.room);
    q.set("signal", inviteFromUrl.signal);
    void goto(`/${role}?${q}`).catch(() => {
      urlAutoNavScheduled = false;
    });
  });

  function pickRoleForInvite(r: "baby" | "parent"): void {
    const p = new URLSearchParams(page.url.searchParams);
    p.set("role", r);
    goto(`/pair?${p}`);
  }

  function signalingForNextPage(): string {
    const s = page.url.searchParams.get("signal");
    if (s && validateSignalingUrl(s) === null) return s;
    return getEffectiveSignalingUrl();
  }

  function goToRoleSession(r: "baby" | "parent", part: ParsedInvite): void {
    error = undefined;
    if (part.pskB64) storePairingMaterial(part.room, part.pskB64);
    const q = new URLSearchParams();
    q.set("room", part.room);
    q.set("signal", part.signal);
    void goto(`/${r}?${q}`);
  }

  function start(): void {
    error = undefined;
    if (role === "parent" && parentJoinDraft.trim() !== "") {
      submitParentJoinInput();
      return;
    }
    const id = normalizeRoomId(roomInput.trim());
    if (!id) {
      error = tWithEnFallback("pair.errors.roomFormat");
      return;
    }
    if (!role) {
      error = tWithEnFallback("pair.errors.roleMissing");
      return;
    }
    const pskStr = invitePsk || getStoredPairingPskB64ForRoom(id) || "";
    if (pskStr) storePairingMaterial(id, pskStr);
    const q = new URLSearchParams();
    q.set("room", id);
    q.set("signal", signalingForNextPage());
    goto(`/${role}?${q}`);
  }

  /** Parent: full invite (link/JSON), partial invite, or room code only + default signaling server. */
  function submitParentJoinInput(): void {
    if (role !== "parent") return;
    error = undefined;
    const t = parentJoinDraft.trim();
    if (!t) return;
    const part = parseInviteFromString(t);
    if (part.room) roomInput = part.room;
    if (isCompleteInvite(part)) {
      goToRoleSession("parent", part);
      parentJoinDraft = "";
      return;
    }
    if (part.room || part.signal) {
      error = tWithEnFallback("pair.errors.inviteIncomplete");
      return;
    }
    const id = normalizeRoomId(t);
    const sig = getEffectiveSignalingUrl();
    if (id && validateSignalingUrl(sig) === null) {
      roomInput = id;
      parentJoinDraft = "";
      start();
      return;
    }
    error = tWithEnFallback("pair.errors.unknownJoinInput");
  }

  function newRoom(): void {
    roomInput = generatePassphraseRoomId();
    invitePsk = generatePskBase64Url();
  }

  function onScannedInvite(part: ParsedInvite): void {
    goToRoleSession("parent", part);
  }
</script>

<div class="mx-auto flex max-w-md flex-col gap-6 px-4 py-10">
  <h1 class="text-xl font-semibold text-zinc-50">{tWithEnFallback("pair.pageTitle")}</h1>

  {#if !role}
    {#if isCompleteInvite(inviteFromUrl)}
      <p class="text-sm text-zinc-400">
        {tWithEnFallback("pair.invite.openedPrompt")}
      </p>
      <div class="flex flex-col gap-2">
        <button
          type="button"
          class="rounded-lg bg-emerald-700 px-4 py-3 font-medium text-white hover:bg-emerald-600"
          onclick={() => pickRoleForInvite("baby")}
        >
          {tWithEnFallback("pair.invite.roleBaby")}
        </button>
        <button
          type="button"
          class="rounded-lg border border-zinc-600 px-4 py-3 font-medium text-zinc-200 hover:bg-zinc-900"
          onclick={() => pickRoleForInvite("parent")}
        >
          {tWithEnFallback("pair.invite.roleParent")}
        </button>
      </div>
    {:else}
      <p class="text-sm text-amber-300">
        {tWithEnFallback("pair.invite.noRolePrefix")}
        <a class="text-emerald-400 underline" href="/">{tWithEnFallback("pair.invite.goHome")}</a>
        {tWithEnFallback("pair.invite.noRoleSuffix")}
      </p>
    {/if}
  {:else}
    {#if role === "baby"}
      <div class="space-y-2 text-sm text-zinc-500">
        <p>
          {tWithEnFallback("pair.baby.intro1")}
        </p>
        <p>
          {tWithEnFallback("pair.baby.intro2")}
        </p>
      </div>
      <div class="flex flex-wrap gap-2">
        <button
          type="button"
          class="rounded-lg bg-zinc-800 px-3 py-2 text-sm text-zinc-100 hover:bg-zinc-700"
          onclick={newRoom}
        >
          {tWithEnFallback("pair.baby.newRoom")}
        </button>
      </div>
    {/if}

    {#if role === "parent"}
      <div
        class="rounded-lg border border-emerald-900/50 bg-emerald-950/20 p-3 text-sm text-zinc-300"
      >
        <p class="font-medium text-emerald-200/90">{tWithEnFallback("pair.parent.stepsTitle")}</p>
        <ol class="mt-2 list-decimal space-y-2 pl-5 text-zinc-400 marker:text-emerald-600/80">
          <li>
            {tWithEnFallback("pair.parent.step1")}
          </li>
          <li>
            {tWithEnFallback("pair.parent.step2")}
          </li>
          <li>
            {tWithEnFallback("pair.parent.step3")}
          </li>
        </ol>
      </div>
    {/if}

    {#if role === "baby"}
      <label class="block space-y-2">
        <span class="text-sm text-zinc-400">{tWithEnFallback("pair.roomCodeLabel")}</span>
        <input
          class="w-full rounded-lg border border-zinc-800 bg-zinc-900 px-3 py-2 font-mono text-zinc-100 outline-none ring-emerald-600/40 focus:ring-2"
          bind:value={roomInput}
          autocomplete="off"
          placeholder={tWithEnFallback("pair.roomPlaceholder")}
        />
      </label>
    {/if}

    {#if showHostQr && qrDataUrl}
      <div class="rounded-xl border border-zinc-800 bg-zinc-950 p-4 text-center">
        {#if loopbackSignalingForQr}
          <div
            class="mb-3 rounded-lg border border-amber-800/70 bg-amber-950/35 px-3 py-2 text-left text-xs text-amber-100/95"
            role="status"
          >
            <p class="font-medium text-amber-200">
              {tWithEnFallback("pair.loopbackTitle")}
            </p>
            <p class="mt-1 text-amber-100/85">
              {tWithEnFallback("pair.loopbackBody")}
            </p>
          </div>
        {/if}
        <p class="mb-2 text-xs text-zinc-500">
          {tWithEnFallback("pair.qrScanHint")}
        </p>
        <img
          src={qrDataUrl}
          alt={tWithEnFallback("pair.qrAlt")}
          class="mx-auto h-auto max-w-[280px] rounded-lg"
        />
      </div>
    {/if}

    {#if role === "parent" && !parentHasInviteInUrl}
      <div class="space-y-3 rounded-lg border border-zinc-800 bg-zinc-900/50 p-3">
        <p class="text-xs font-medium text-zinc-400">{tWithEnFallback("pair.parent.joinTitle")}</p>
        <div class="space-y-1">
          <label for="parent-join-field" class="text-xs font-medium text-zinc-500"
            >{tWithEnFallback("pair.parent.joinLabel")}</label
          >
          <input
            id="parent-join-field"
            data-testid="parent-join-field"
            type="text"
            autocomplete="off"
            spellcheck="false"
            placeholder={tWithEnFallback("pair.parent.joinPlaceholder")}
            bind:value={parentJoinDraft}
            class="w-full rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 font-mono text-xs text-zinc-100 placeholder:text-zinc-600 focus:border-emerald-600 focus:outline-none"
            onkeydown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                submitParentJoinInput();
              }
            }}
          />
          <p class="text-[11px] text-zinc-600">
            {tWithEnFallback("pair.parent.joinHint")}
          </p>
        </div>
        <InviteScanner onInvite={onScannedInvite} />
      </div>

      <details
        class="rounded-lg border border-zinc-800 bg-zinc-900/30"
        bind:open={parentInviteOpen}
      >
        <summary
          class="cursor-pointer select-none px-3 py-3 text-sm text-zinc-300 marker:text-zinc-500"
        >
          {tWithEnFallback("pair.parent.inviteAnotherSummary")}
        </summary>
        <div class="space-y-3 border-t border-zinc-800 px-3 pb-3 pt-3 text-sm text-zinc-500">
          <p>
            {tWithEnFallback("pair.parent.inviteAnotherBody")}
          </p>
          <div class="flex flex-wrap gap-2">
            <button
              type="button"
              class="rounded-lg bg-zinc-800 px-3 py-2 text-sm text-zinc-100 hover:bg-zinc-700"
              onclick={newRoom}
            >
              {tWithEnFallback("pair.parent.inviteAnotherNew")}
            </button>
          </div>
        </div>
      </details>
    {/if}

    {#if error}
      <p class="text-sm text-amber-300">{error}</p>
    {/if}
    <button
      type="button"
      class="rounded-lg bg-emerald-600 px-4 py-2 font-medium text-emerald-950 hover:bg-emerald-500"
      data-testid="pair-start"
      onclick={start}
    >
      {#if role === "baby"}
        {tWithEnFallback("pair.startBaby")}
      {:else}
        {tWithEnFallback("pair.parent.startListen")}
      {/if}
    </button>
  {/if}
</div>
