<script lang="ts">
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";
  import { readAppPreferences } from "$lib/app-preferences.js";
  import { prewarmBabyMicrophoneFromUserGesture } from "$lib/audio/baby-microphone.js";
  import RolePicker from "$lib/components/RolePicker.svelte";

  onMount(() => {
    if (!readAppPreferences().onboardingCompleted) void goto("/onboard");
  });

  async function pickRole(role: "baby" | "parent"): Promise<void> {
    if (role === "baby") {
      await prewarmBabyMicrophoneFromUserGesture();
      void goto("/baby");
    } else {
      void goto("/pair?role=parent");
    }
  }
</script>

<RolePicker onPick={(role) => void pickRole(role)} />
