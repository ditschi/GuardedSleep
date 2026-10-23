import { getContext, setContext } from "svelte";
import type { AppController } from "./controller";

const KEY = Symbol("guardedsleep.app-controller");

/** App root: hands the platform's controller to every screen below it. */
export function provideController(controller: AppController): void {
  setContext(KEY, controller);
}

/** Screens and components: the controller provided by the app root. */
export function useController(): AppController {
  const controller = getContext<AppController | undefined>(KEY);
  if (!controller) throw new Error("no AppController provided: call provideController at the root");
  return controller;
}
