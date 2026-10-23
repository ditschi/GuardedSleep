import { mount, unmount } from "svelte";
import { describe, expect, it } from "vitest";
import App from "./App.svelte";

describe("app shell", () => {
  it("mounts the main landmark", () => {
    const target = document.createElement("div");
    document.body.append(target);
    const app = mount(App, { target });
    expect(target.querySelector("main#main")).not.toBeNull();
    void unmount(app);
    target.remove();
  });
});
