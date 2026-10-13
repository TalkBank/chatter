/**
 * Smoke tests for Chatter Desktop.
 *
 * These tests launch the real app via tauri-driver and check launch UI
 * through the WebDriver protocol. They are slow (~5-10s each)
 * and require a built app binary, so they run separately from unit tests.
 *
 * Run:
 *   1. cargo tauri build --debug
 *   2. tauri-driver &          (start in background, listens on :4444)
 *   3. npx wdio run wdio.conf.ts
 */

describe("Chatter Desktop", () => {
  it("should launch and show the drop zone", async () => {
    // The drop zone area should be visible on launch
    const dropZone = await $(".drop-zone");
    await expect(dropZone).toBeDisplayed();
  });

  it("should show Choose File and Choose Folder buttons", async () => {
    const buttons = await $$(".drop-zone button");
    expect(buttons.length).toBeGreaterThanOrEqual(2);

    const texts = await Promise.all(buttons.map((b) => b.getText()));
    expect(texts).toContain("Choose File");
    expect(texts).toContain("Choose Folder");
  });

  it("should show empty file tree on launch", async () => {
    const tree = await $(".file-tree-panel");
    await expect(tree).toBeDisplayed();

    const text = await tree.getText();
    expect(text).toContain("No files loaded");
  });

  it("should show empty error panel on launch", async () => {
    const panel = await $(".error-panel");
    await expect(panel).toBeDisplayed();

    const text = await panel.getText();
    expect(text).toContain("Select a file to view errors");
  });

  it("should show idle status bar", async () => {
    const statusBar = await $(".status-bar");
    await expect(statusBar).toBeDisplayed();

    const text = await statusBar.getText();
    expect(text).toContain("Ready");
  });

  // Launch checks are not native validation-flow evidence. Future interaction
  // tests must use the production typed contract, bounded terminal events and
  // isolated state; see the book's Desktop App Testing chapter.
});
