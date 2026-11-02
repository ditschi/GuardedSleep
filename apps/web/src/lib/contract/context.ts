import { getContext, setContext } from "svelte";
import type { AppController } from "./controller";
import type { CodeScanner } from "./scanner";

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

const SCANNER_KEY = Symbol("guardedsleep.code-scanner");

/** App root: hands over the camera scanner; without one the pairing screen offers paste only. */
export function provideScanner(scanner: CodeScanner | undefined): void {
  setContext(SCANNER_KEY, scanner);
}

export function useScanner(): CodeScanner | undefined {
  return getContext<CodeScanner | undefined>(SCANNER_KEY);
}
