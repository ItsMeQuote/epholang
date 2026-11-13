import { test, expect } from "@playwright/test";
import AxeBuilder from "@axe-core/playwright";
test("root language choices and content work without JavaScript", async ({
  browser,
}) => {
  const context = await browser.newContext({ javaScriptEnabled: false });
  const page = await context.newPage();
  await page.goto("/epholang/");
  await expect(
    page.getByRole("link", { name: "Русский", exact: false }).last(),
  ).toHaveAttribute("href", "/epholang/ru/");
  await page.locator('[data-choice="ru"]').click();
  await expect(page.locator("html")).toHaveAttribute("lang", "ru");
  await expect(page.locator("[data-panel]")).toHaveCount(3);
  for (const p of await page.locator("[data-panel]").all())
    await expect(p).toBeVisible();
  await context.close();
});
test("all localized pages, metadata and accessibility", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  for (const lang of ["ru", "en"])
    for (const slug of [
      "",
      "getting-started/",
      "docs/",
      "examples/",
      "download/",
      "about/",
    ]) {
      await page.goto(`/epholang/${lang}/${slug}`);
      await expect(page.locator("h1")).toHaveCount(1);
      await expect(page.locator("html")).toHaveAttribute("lang", lang);
      expect(
        (
          await new AxeBuilder({ page })
            .withTags(["wcag2a", "wcag2aa", "wcag21aa", "wcag22aa"])
            .analyze()
        ).violations,
      ).toEqual([]);
    }
  expect(errors).toEqual([]);
});
test("language preserves route, remembers preference and tolerates blocked storage", async ({
  page,
}) => {
  await page.goto("/epholang/ru/examples/");
  await page.locator('[data-language="en"]').click();
  await expect(page).toHaveURL(/en\/examples\//);
  await page.goto("/epholang/");
  await expect(page).toHaveURL(/en\/$/);
  await page.addInitScript(() => {
    Object.defineProperty(window, "localStorage", {
      get() {
        throw new Error("blocked");
      },
    });
  });
  await page.goto("/epholang/en/docs/");
  await page.locator('[data-language="ru"]').click();
  await expect(page).toHaveURL(/ru\/docs\//);
});
test("keyboard tabs and copy", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto("/epholang/en/");
  const tabs = page.getByRole("tab");
  await tabs.first().focus();
  await page.keyboard.press("ArrowRight");
  await expect(tabs.nth(1)).toHaveAttribute("aria-selected", "true");
  await page
    .getByRole("tabpanel")
    .filter({ visible: true })
    .getByRole("button", { name: "Copy code" })
    .click();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toContain(
    "Hello, Epholang!",
  );
});
test("mobile menu, Escape and responsive overflow", async ({ page }) => {
  for (const width of [320, 390, 768, 1280]) {
    await page.setViewportSize({ width, height: 900 });
    for (const route of [
      "ru/",
      "ru/getting-started/",
      "en/examples/",
      "ru/download/",
    ]) {
      await page.goto("/epholang/" + route);
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= window.innerWidth,
        ),
      ).toBe(true);
    }
    if (width < 850) {
      const menu = page.getByRole("button", { name: "Меню навигации" });
      await menu.click();
      await expect(menu).toHaveAttribute("aria-expanded", "true");
      await page.keyboard.press("Escape");
      await expect(menu).toBeFocused();
      await expect(menu).toHaveAttribute("aria-expanded", "false");
    }
  }
});

test('text enlargement and clipboard failure remain usable',async({page})=>{
 await page.setViewportSize({width:390,height:844});
 for(const route of ['ru/','ru/getting-started/','ru/download/']){
  await page.goto('/epholang/'+route);
  await page.evaluate(()=>document.documentElement.style.fontSize='200%');
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
 }
 await page.goto('/epholang/en/');
 await page.evaluate(()=>Object.defineProperty(navigator,'clipboard',{value:{writeText:()=>Promise.reject(new Error('denied'))}}));
 await page.getByRole('tabpanel').filter({visible:true}).getByRole('button',{name:'Copy code'}).click();
 await expect(page.getByRole('status').filter({hasText:'Select and copy'})).toBeVisible();
});
